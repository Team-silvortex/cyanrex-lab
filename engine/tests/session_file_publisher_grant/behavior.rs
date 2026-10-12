use super::support::*;
use crate::{fixture::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_publisher_grant_own_metadata_is_not_role_or_current_permission() {
    let f = Fixture::ready().await;
    let own = f.issue(1, f.subject()).await;
    let manager = f.issue(2, f.issuer()).await;
    let wrong_generation = FileAccountRef::new(f.subject().principal(), id());
    let historical = f.issue(3, wrong_generation).await;
    assert_ne!(historical.grant().subject(), f.subject());
    assert_eq!(
        read(&f, &f.auth.learner_token, 1).await.unwrap(),
        Some(own.clone())
    );
    assert_eq!(
        read(&f, &f.auth.manager_token, 1).await.unwrap(),
        None,
        "deployment manager cannot read another subject's grant"
    );
    assert_eq!(read(&f, &f.auth.learner_token, 2).await.unwrap(), None);
    assert_eq!(
        read(&f, &f.auth.manager_token, 2).await.unwrap(),
        Some(manager)
    );
    assert_eq!(
        read(&f, &f.auth.learner_token, 3).await.unwrap(),
        None,
        "same Principal but another account incarnation remains private"
    );
    let current = f.auth.current(f.auth.learner.binding.principal_id).await;
    let mut desired = current.policy;
    desired.membership.role_refs = vec!["cyanrex.teaching.teacher".parse().unwrap()];
    f.auth
        .source
        .apply_session_policy_command(
            &f.auth.manager_token,
            &SessionPolicyCommand {
                command_id: id(),
                expected_revision: Some(current.revision),
                desired,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        read(&f, &f.auth.learner_token, 2).await.unwrap(),
        None,
        "teacher is not a grant administrator or another subject"
    );
    assert_eq!(read(&f, &f.auth.learner_token, 1).await.unwrap(), Some(own));
    let active = f.issue(4, f.subject()).await;
    let revoked = f
        .store
        .revoke(grant(4), f.issuer(), active.grant().revision())
        .await
        .unwrap();
    let expired = f
        .store
        .issue(
            grant(5),
            f.issuer(),
            f.subject(),
            FileGrantValidity::new(1, 2).unwrap(),
        )
        .await
        .unwrap();
    let future = f
        .store
        .issue(
            grant(6),
            f.issuer(),
            f.subject(),
            FileGrantValidity::new(MAX_FILE_GRANT_UNIX_SECONDS - 1, MAX_FILE_GRANT_UNIX_SECONDS)
                .unwrap(),
        )
        .await
        .unwrap();
    for (seed, expected) in [(4, revoked), (5, expired), (6, future)] {
        assert!(!expected
            .grant()
            .matches(scope(), f.subject(), Utc::now().timestamp() as u64));
        assert_eq!(read(&f, &f.auth.learner_token, seed).await.unwrap(), Some(expected), "revoked/expired/not-yet-valid own metadata is readable without authorizing publication");
    }
    assert_eq!(read(&f, &f.auth.learner_token, 99).await.unwrap(), None);
    let original = read(&f, &f.auth.learner_token, 1).await.unwrap();
    f.pool.close().await;
    assert_eq!(read(&f, &f.auth.learner_token, 1).await.unwrap(), original, "Session GET must use the source-owned transaction, not the closed independent installation pool");
    f.cleanup().await;
}
