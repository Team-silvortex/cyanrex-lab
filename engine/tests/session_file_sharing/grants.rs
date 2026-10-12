use super::support::*;
use crate::{fixture::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_sharing_requires_current_own_active_publisher_permission_but_not_for_revoke(
) {
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            for (seed, subject, v) in [
                (2, f.actor(), FileGrantValidity::new(1, 2).unwrap()),
                (
                    3,
                    f.actor(),
                    FileGrantValidity::new(
                        MAX_FILE_GRANT_UNIX_SECONDS - 1,
                        MAX_FILE_GRANT_UNIX_SECONDS,
                    )
                    .unwrap(),
                ),
                (4, f.base.grants.issuer(), grant_fixture::validity()),
                (
                    5,
                    FileAccountRef::new(f.actor().principal(), id()),
                    grant_fixture::validity(),
                ),
                (6, f.actor(), grant_fixture::validity()),
            ] {
                f.base
                    .grants
                    .store
                    .issue(
                        grant_fixture::grant(seed),
                        f.base.grants.issuer(),
                        subject,
                        v,
                    )
                    .await
                    .unwrap();
            }
            f.base
                .grants
                .store
                .revoke(grant_fixture::grant(6), f.base.grants.issuer(), revision(1))
                .await
                .unwrap();
            for seed in [2, 3, 4, 5, 6, 99] {
                let mut c = f.command(1);
                c.publisher_grant_id = grant_fixture::grant(seed);
                assert_eq!(
                    issue_as(f, &f.base.grants.auth.learner_token, &f.workspace, &c).await,
                    Err(SessionFileSharingError::PermissionDenied)
                );
            }
            let old = issue(f, 1).await.unwrap();
            let mut future = f.command(2);
            future.validity = FileGrantValidity::new(
                MAX_FILE_GRANT_UNIX_SECONDS - 1,
                MAX_FILE_GRANT_UNIX_SECONDS,
            )
            .unwrap();
            let pending = issue_as(f, &f.base.grants.auth.learner_token, &f.workspace, &future)
                .await
                .unwrap();
            assert_eq!(
                pending.grant().validity(),
                future.validity,
                "recipient metadata windows need not contain now"
            );
            f.base
                .grants
                .store
                .revoke(grant_fixture::grant(1), f.base.grants.issuer(), revision(1))
                .await
                .unwrap();
            assert_eq!(
                get(f, 1, f.recipient).await.unwrap(),
                Some(old.clone()),
                "publisher revocation never cascades"
            );
            assert_eq!(
                issue(f, 3).await,
                Err(SessionFileSharingError::PermissionDenied)
            );
            let stopped = revoke(f, 1, revision(1)).await.unwrap();
            assert_eq!(stopped.grant().state(), FileGrantState::Revoked);
            assert_eq!(stopped.issuer(), old.issuer());
        })
    })
    .await;
}
