use super::cleanup::checked;
use super::{fixture::*, support::*, *};
use cyanrex_engine::services::file_publisher_grant_store::FilePublisherGrantStore;
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_recipient_grants_bind_exact_parties_file_and_independent_publisher_ledger() {
    checked(ready(16).await, |f| {
        Box::pin(async move {
            let first = issue(&f, 1, account(1), account(2), account(3), validity())
                .await
                .unwrap();
            assert!(first.grant().matches(&file(1), account(2), account(3), 150));
            assert!(!first.grant().matches(&file(1), account(2), account(3), 200));
            for other in [
                account(1),
                account(2),
                account(4),
                FileAccountRef::new(account(3).principal(), account(4).account_id()),
                FileAccountRef::new(account(4).principal(), account(3).account_id()),
            ] {
                assert_eq!(get(&f, 1, other).await.unwrap(), None);
            }
            assert_eq!(get(&f, 99, account(3)).await.unwrap(), None);
            assert_eq!(get(&f, 1, account(3)).await.unwrap(), Some(first.clone()));
            let generation = FileAccountRef::new(account(3).principal(), account(4).account_id());
            let second = issue_file(
                &f,
                2,
                account(1),
                file(1),
                account(2),
                generation,
                validity(),
            )
            .await
            .unwrap();
            assert_eq!(get(&f, 2, account(3)).await.unwrap(), None);
            assert_eq!(get(&f, 2, generation).await.unwrap(), Some(second));
            issue_file(
                &f,
                3,
                account(1),
                file(1),
                account(2),
                account(3),
                validity(),
            )
            .await
            .unwrap();
            let self_recipient = issue_file(
                &f,
                4,
                account(1),
                file(1),
                account(2),
                account(2),
                validity(),
            )
            .await
            .unwrap();
            assert_eq!(get(&f, 4, account(2)).await.unwrap(), Some(self_recipient));
            assert_eq!(
                issue_file(
                    &f,
                    1,
                    account(8),
                    file(8),
                    account(8),
                    account(8),
                    FileGrantValidity::new(200, 300).unwrap()
                )
                .await,
                Err(FileRecipientGrantStoreError::Conflict)
            );
            // Publisher authorization is independent: a revoked publisher grant does not cascade.
            // No Registry, Published row, file bytes, or download permission are asserted here.
            let (schema, pool) = f.side_namespace().await;
            let store =
                FilePublisherGrantStore::new(pool.clone(), scope(), Uuid::new_v4(), &schema, 4)
                    .unwrap();
            store.install_empty_namespace().await.unwrap();
            let before = f.snapshot().await;
            store
                .issue(grant(99), account(1), account(2), validity())
                .await
                .unwrap();
            store
                .revoke(grant(99), account(4), revision(1))
                .await
                .unwrap();
            assert_eq!(f.snapshot().await, before);
            assert_eq!(get(&f, 1, account(3)).await.unwrap(), Some(first.clone()));
            assert!(first.grant().matches(&file(1), account(2), account(3), 150));
            pool.close().await;
            restored(&f).await;
        })
    })
    .await;
}
