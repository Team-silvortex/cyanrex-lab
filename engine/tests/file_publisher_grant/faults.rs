use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_publisher_grant_suppressed_or_tampered_issue_and_revoke_roll_back() {
    for table in [HEADS, CHANGES] {
        let f = ready(8).await;
        install_trigger(&f, table, "INSERT", "RETURN NULL;", false).await;
        let result = issue(&f, 1, account(1), account(2), validity()).await;
        let fired = hits(&f).await;
        drop_trigger(&f, table).await;
        assert!(get(&f, 1, account(2)).await.unwrap().is_none());
        issue(&f, 1, account(1), account(2), validity())
            .await
            .unwrap();
        f.cleanup().await;
        assert!(fired);
        assert_eq!(result, Err(FilePublisherGrantStoreError::InvalidRecord));
    }
    for (table, event) in [(HEADS, "UPDATE"), (CHANGES, "INSERT")] {
        let f = ready(8).await;
        let original = issue(&f, 1, account(1), account(2), validity())
            .await
            .unwrap();
        install_trigger(&f, table, event, "RETURN NULL;", false).await;
        let result = revoke(&f, 1, account(9), revision(1)).await;
        let fired = hits(&f).await;
        drop_trigger(&f, table).await;
        assert_eq!(get(&f, 1, account(2)).await.unwrap(), Some(original));
        revoke(&f, 1, account(9), revision(1)).await.unwrap();
        f.cleanup().await;
        assert!(fired);
        assert_eq!(result, Err(FilePublisherGrantStoreError::InvalidRecord));
    }
    let f = ready(8).await;
    install_trigger(
        &f,
        CHANGES,
        "INSERT",
        &format!(
            "NEW.actor_account_id := '{}'; RETURN NEW;",
            account(8).account_id()
        ),
        false,
    )
    .await;
    let result = issue(&f, 1, account(1), account(2), validity()).await;
    let fired = hits(&f).await;
    drop_trigger(&f, CHANGES).await;
    issue(&f, 1, account(1), account(2), validity())
        .await
        .unwrap();
    f.cleanup().await;
    assert!(fired);
    assert_eq!(result, Err(FilePublisherGrantStoreError::InvalidRecord));
}
