use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_publisher_grant_foreign_corruption_and_excess_inventory_precede_conflict_and_quota(
) {
    let f = ready(2).await;
    let own = issue(&f, 1, account(1), account(1), validity())
        .await
        .unwrap();
    issue(&f, 2, account(2), account(2), validity())
        .await
        .unwrap();
    let original = head_payload(&f, 2).await;
    update_payload(&f, HEADS, 2, 1, b"{}").await;
    for seed in [1, 3] {
        assert_eq!(
            issue(&f, seed, account(1), account(1), validity()).await,
            Err(FilePublisherGrantStoreError::InvalidRecord)
        );
    }
    assert_eq!(
        revoke(&f, 1, account(9), revision(1)).await,
        Err(FilePublisherGrantStoreError::InvalidRecord)
    );
    // Selected trusted metadata is intentionally not an audit of somebody else's ledger.
    assert_eq!(get(&f, 1, account(1)).await.unwrap(), Some(own));
    update_payload(&f, HEADS, 2, 1, &original).await;
    assert_eq!(
        issue(&f, 1, account(1), account(1), validity()).await,
        Err(FilePublisherGrantStoreError::Conflict)
    );
    assert_eq!(
        issue(&f, 3, account(3), account(3), validity()).await,
        Err(FilePublisherGrantStoreError::QuotaExceeded)
    );
    insert_raw(&f, 3, account(3), account(3)).await;
    assert_eq!(
        issue(&f, 1, account(1), account(1), validity()).await,
        Err(FilePublisherGrantStoreError::InvalidRecord)
    );
    assert_eq!(
        issue(&f, 4, account(4), account(4), validity()).await,
        Err(FilePublisherGrantStoreError::InvalidRecord)
    );
    sql(
        &f,
        &format!("DELETE FROM {CHANGES} WHERE grant_id='{}'", grant(3)),
    )
    .await;
    sql(
        &f,
        &format!("DELETE FROM {HEADS} WHERE grant_id='{}'", grant(3)),
    )
    .await;
    // A valid historical payload with no corresponding head must not disappear from a
    // head-only inventory. The writer audits all history coordinates, including orphans.
    query(&format!("INSERT INTO {CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) SELECT $1,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload FROM {CHANGES} WHERE grant_id=$2"))
        .bind(grant(99).as_uuid()).bind(grant(2).as_uuid()).execute(&f.pool).await.unwrap();
    assert_eq!(
        issue(&f, 1, account(1), account(1), validity()).await,
        Err(FilePublisherGrantStoreError::InvalidRecord)
    );
    sql(
        &f,
        &format!("DELETE FROM {CHANGES} WHERE grant_id='{}'", grant(99)),
    )
    .await;
    revoke(&f, 1, account(9), revision(1)).await.unwrap();
    f.cleanup().await;

    let f = ready(256).await;
    for seed in 1..=257 {
        insert_raw(&f, seed, account(seed), account(seed)).await;
    }
    let before = snapshot(&f).await;
    let excess = f
        .store
        .issue(grant(999), account(999), account(999), validity())
        .await;
    assert_eq!(snapshot(&f).await, before);
    f.cleanup().await;
    assert_eq!(excess, Err(FilePublisherGrantStoreError::InvalidRecord));
}
