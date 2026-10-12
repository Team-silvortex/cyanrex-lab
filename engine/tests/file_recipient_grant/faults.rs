use super::cleanup::checked;
use super::{fixture::*, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_recipient_suppressed_or_tampered_issue_revoke_roll_back_exactly() {
    for (table, event, revoke_case) in [
        (HEADS, "INSERT", false),
        (CHANGES, "INSERT", false),
        (HEADS, "UPDATE", true),
        (CHANGES, "INSERT", true),
    ] {
        checked(ready(8).await, |f| {
            Box::pin(async move {
                let original = if revoke_case {
                    Some(
                        issue(&f, 1, account(1), account(2), account(3), validity())
                            .await
                            .unwrap(),
                    )
                } else {
                    None
                };
                install_trigger(&f, table, event, "RETURN NULL;", false).await;
                let failed = if revoke_case {
                    revoke(&f, 1, account(9), revision(1)).await
                } else {
                    issue(&f, 1, account(1), account(2), account(3), validity()).await
                };
                let fired = hits(&f).await;
                drop_trigger(&f, table).await;
                assert_eq!(get(&f, 1, account(3)).await.unwrap(), original);
                if revoke_case {
                    revoke(&f, 1, account(9), revision(1)).await.unwrap();
                } else {
                    issue(&f, 1, account(1), account(2), account(3), validity())
                        .await
                        .unwrap();
                }
                assert!(fired);
                assert_eq!(failed, Err(FileRecipientGrantStoreError::InvalidRecord));
            })
        })
        .await;
    }
    for (table, body) in [
        (
            CHANGES,
            format!(
                "NEW.actor_account_id:='{}'; RETURN NEW;",
                account(8).account_id()
            ),
        ),
        (
            HEADS,
            format!(
                "NEW.publisher_account_id:='{}'; RETURN NEW;",
                account(8).account_id()
            ),
        ),
        (
            CHANGES,
            "RAISE EXCEPTION 'synthetic immediate error';".into(),
        ),
    ] {
        checked(ready(8).await, |f| {
            Box::pin(async move {
                install_trigger(&f, table, "INSERT", &body, false).await;
                let failed = issue(&f, 1, account(1), account(2), account(3), validity()).await;
                let fired = hits(&f).await;
                drop_trigger(&f, table).await;
                issue(&f, 1, account(1), account(2), account(3), validity())
                    .await
                    .unwrap();
                assert!(fired);
                let expected = if body.starts_with("RAISE") {
                    FileRecipientGrantStoreError::StorageUnavailable
                } else {
                    FileRecipientGrantStoreError::InvalidRecord
                };
                assert_eq!(failed, Err(expected));
            })
        })
        .await;
    }
}
