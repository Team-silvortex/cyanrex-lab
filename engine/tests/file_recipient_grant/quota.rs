use super::cleanup::checked;
use super::{fixture::*, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_recipient_expired_revoked_and_account_generations_retain_global_quota() {
    checked(ready(3).await, |f| {
        Box::pin(async move {
            let expired = issue(
                &f,
                1,
                account(1),
                account(2),
                account(3),
                FileGrantValidity::new(0, 1).unwrap(),
            )
            .await
            .unwrap();
            assert_eq!(get(&f, 1, account(3)).await.unwrap(), Some(expired));
            let next = FileAccountRef::new(account(3).principal(), account(4).account_id());
            issue(&f, 2, account(1), account(2), next, validity())
                .await
                .unwrap();
            let retired = revoke(&f, 2, account(9), revision(1)).await.unwrap();
            let third = issue(&f, 3, account(4), account(5), account(6), validity())
                .await
                .unwrap();
            assert_eq!(
                issue(&f, 4, account(1), account(2), account(3), validity()).await,
                Err(FileRecipientGrantStoreError::QuotaExceeded)
            );
            for seed in [1, 2, 3] {
                assert_eq!(
                    issue(&f, seed, account(1), account(2), account(3), validity()).await,
                    Err(FileRecipientGrantStoreError::Conflict)
                );
            }
            assert_eq!(get(&f, 2, next).await.unwrap(), Some(retired));
            assert_eq!(get(&f, 3, account(6)).await.unwrap(), Some(third));
            let s = f.snapshot().await;
            assert_eq!(rows(&s, HEADS).len(), 3);
            assert_eq!(rows(&s, CHANGES).len(), 4);
            restored(&f).await;
        })
    })
    .await;
}
