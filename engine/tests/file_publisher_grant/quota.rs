use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_expired_revoked_and_cross_generation_grants_retain_global_quota() {
    let f = ready(3).await;
    let expired = issue(
        &f,
        1,
        account(1),
        account(2),
        FileGrantValidity::new(0, 1).unwrap(),
    )
    .await
    .unwrap();
    assert!(!expired.grant().matches(scope(), account(2), 1));
    assert_eq!(get(&f, 1, account(2)).await.unwrap(), Some(expired));
    let next_generation = FileAccountRef::new(account(2).principal(), account(3).account_id());
    issue(&f, 2, account(1), next_generation, validity())
        .await
        .unwrap();
    let retained = revoke(&f, 2, account(9), revision(1)).await.unwrap();
    let third = issue(&f, 3, account(4), account(5), validity())
        .await
        .unwrap();
    assert_eq!(
        issue(&f, 4, account(1), account(2), validity()).await,
        Err(FilePublisherGrantStoreError::QuotaExceeded)
    );
    assert_eq!(
        issue(&f, 3, account(4), account(5), validity()).await,
        Err(FilePublisherGrantStoreError::Conflict),
        "an occupied ID conflicts even after quota is full"
    );
    assert_eq!(
        issue(&f, 2, account(1), next_generation, validity()).await,
        Err(FilePublisherGrantStoreError::Conflict),
        "revoked IDs cannot be adopted or reactivated"
    );
    assert_eq!(get(&f, 2, next_generation).await.unwrap(), Some(retained));
    assert_eq!(get(&f, 3, account(5)).await.unwrap(), Some(third));
    // No automatic expiration/deletion/release operation: three retained IDs still consume cap.
    let snapshot = snapshot(&f).await;
    assert_eq!(rows(&snapshot, HEADS).len(), 3);
    assert_eq!(rows(&snapshot, CHANGES).len(), 4);
    restored(&f).await;
    f.cleanup().await;
}
