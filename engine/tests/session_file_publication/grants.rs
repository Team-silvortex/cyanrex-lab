use super::{fixture::*, support::*, *};
use std::{fs, os::unix::fs::PermissionsExt};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_publication_requires_current_exact_subject_generation_and_active_half_open_grant(
) {
    for mode in [
        "missing",
        "foreign-principal",
        "foreign-account",
        "expired",
        "future",
        "revoked",
        "wrong-actor",
    ] {
        let f = Fixture::ready(17).await;
        let actor = f.actor();
        let now: u64 = query("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint AS n")
            .fetch_one(&f.grants.auth.base.admin)
            .await
            .unwrap()
            .get::<i64, _>("n") as u64;
        if mode != "missing" {
            let subject = match mode {
                "foreign-principal" => FileAccountRef::new(
                    PrincipalRef {
                        principal_id: id(),
                        ..actor.principal()
                    },
                    actor.account_id(),
                ),
                "foreign-account" => FileAccountRef::new(actor.principal(), id()),
                _ => actor,
            };
            let v = match mode {
                "expired" => FileGrantValidity::new(0, now).unwrap(),
                "future" => FileGrantValidity::new(now + 300, now + 600).unwrap(),
                _ => grant_fixture::validity(),
            };
            f.grants
                .store
                .issue(grant_fixture::grant(1), f.grants.issuer(), subject, v)
                .await
                .unwrap();
            if mode == "revoked" {
                f.grants
                    .store
                    .revoke(
                        grant_fixture::grant(1),
                        f.grants.issuer(),
                        1u64.try_into().unwrap(),
                    )
                    .await
                    .unwrap();
            }
        }
        let mut a = prepare(&f, 1).await;
        // A corrupt root is deliberately present: an absent/invalid grant must deny BEFORE file work.
        fs::set_permissions(&f.root, fs::Permissions::from_mode(0o770)).unwrap();
        let before = f.snapshot().await;
        let token = if mode == "wrong-actor" {
            &f.grants.auth.manager_token
        } else {
            &f.grants.auth.learner_token
        };
        let result = publish_as(&f, token, &f.workspace, 1, &mut a).await;
        let state = a.state();
        let after = f.snapshot().await;
        let repeated = publish(&f, &mut a).await;
        fs::set_permissions(&f.root, fs::Permissions::from_mode(0o700)).unwrap();
        f.grants.issue(2, actor).await;
        let mut fresh = prepare(&f, 1).await;
        let healthy = publish_as(
            &f,
            &f.grants.auth.learner_token,
            &f.workspace,
            2,
            &mut fresh,
        )
        .await;
        f.cleanup().await;
        assert_eq!(
            result.err(),
            Some(SessionFilePublishingError::PermissionDenied),
            "{mode}"
        );
        assert_eq!(state, FileUploadPublicationState::Unknown);
        assert_eq!(after, before);
        assert_eq!(
            repeated.err(),
            Some(publisher_error(FileUploadPublicationError::InvalidInput))
        );
        assert!(healthy.is_ok(), "healthy independent attempt {mode}");
    }
    super::grant_gap::original_current_grant_is_not_recaptured_after_the_file_gap().await;
}
