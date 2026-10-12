use super::support::*;
use crate::{fixture::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_recipient_metadata_is_private_not_role_time_or_download_authority() {
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let own = f.issue(1, f.recipient()).await;
            let manager = f.issue(2, f.issuer()).await;
            let other_generation = FileAccountRef::new(f.recipient().principal(), id());
            f.issue(3, other_generation).await;
            assert_eq!(
                read(f, &f.auth.learner_token, 1).await.unwrap(),
                Some(own.clone())
            );
            assert_eq!(read(f, &f.auth.manager_token, 1).await.unwrap(), None);
            assert_eq!(read(f, &f.auth.learner_token, 2).await.unwrap(), None);
            assert_eq!(
                read(f, &f.auth.manager_token, 2).await.unwrap(),
                Some(manager)
            );
            assert_eq!(read(f, &f.auth.learner_token, 3).await.unwrap(), None);
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
            assert_eq!(read(f, &f.auth.learner_token, 2).await.unwrap(), None);
            let active = f.issue(4, f.recipient()).await;
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
                    ledger_fixture::file_for(scope(), 5),
                    f.publisher(),
                    f.recipient(),
                    FileGrantValidity::new(1, 2).unwrap(),
                )
                .await
                .unwrap();
            let future = f
                .store
                .issue(
                    grant(6),
                    f.issuer(),
                    ledger_fixture::file_for(scope(), 6),
                    f.publisher(),
                    f.recipient(),
                    FileGrantValidity::new(
                        MAX_FILE_GRANT_UNIX_SECONDS - 1,
                        MAX_FILE_GRANT_UNIX_SECONDS,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            for (seed, record) in [(4, revoked), (5, expired), (6, future)] {
                assert!(!record.grant().matches(
                    record.grant().file(),
                    record.grant().publisher(),
                    f.recipient(),
                    Utc::now().timestamp() as u64
                ));
                assert_eq!(
                    read(f, &f.auth.learner_token, seed).await.unwrap(),
                    Some(record),
                    "metadata access does not authorize download or enforce grant-window matches"
                );
            }
            let self_recipient = f
                .store
                .issue(
                    grant(7),
                    f.issuer(),
                    ledger_fixture::file_for(scope(), 7),
                    f.recipient(),
                    f.recipient(),
                    validity(),
                )
                .await
                .unwrap();
            assert_eq!(
                read(f, &f.auth.learner_token, 7).await.unwrap(),
                Some(self_recipient)
            );
            assert_eq!(read(f, &f.auth.learner_token, 99).await.unwrap(), None);
            let before = f.snapshot().await;
            let original = read(f, &f.auth.learner_token, 1).await.unwrap();
            f.pool.close().await;
            assert_eq!(read(f, &f.auth.learner_token, 1).await.unwrap(), original);
            assert_eq!(f.snapshot().await, before);
        })
    })
    .await;
}
