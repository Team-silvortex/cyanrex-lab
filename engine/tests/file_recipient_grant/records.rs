use super::cleanup::checked;
use super::{fixture::*, support::*, *};
async fn bad(f: &Fixture) {
    assert_eq!(
        get(f, 1, account(3)).await,
        Err(FileRecipientGrantStoreError::InvalidRecord)
    );
    assert_eq!(
        issue(f, 9, account(9), account(9), account(9), validity()).await,
        Err(FileRecipientGrantStoreError::InvalidRecord)
    );
}
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_recipient_canonical_head_full_file_and_each_account_pin_corruption_is_rejected() {
    checked(ready(16).await, |f| {
        Box::pin(async move {
            let own = issue(&f, 1, account(1), account(2), account(3), validity())
                .await
                .unwrap();
            let original = head_payload(&f, 1).await;
            for raw in [b"not JSON".to_vec(), [original.as_slice(), b" "].concat()] {
                update_payload(&f, HEADS, 1, 1, &raw).await;
                bad(&f).await;
                update_payload(&f, HEADS, 1, 1, &original).await;
                assert_eq!(get(&f, 1, account(3)).await.unwrap(), Some(own.clone()));
            }
            for (field, value) in [
                ("file_id", serde_json::json!(file(2).file_id())),
                ("version_id", serde_json::json!(file(2).version_id())),
                ("byte_length", serde_json::json!(12)),
                ("sha256", serde_json::json!("b".repeat(64))),
            ] {
                let mut g = serde_json::to_value(own.grant()).unwrap();
                g["file"][field] = value;
                let changed: FileRecipientGrant = serde_json::from_value(g).unwrap();
                update_payload(&f, HEADS, 1, 1, &serde_json::to_vec(&changed).unwrap()).await;
                bad(&f).await;
                update_payload(&f, HEADS, 1, 1, &original).await;
            }
            let mut g = serde_json::to_value(own.grant()).unwrap();
            g["file"]["workspace"]["workspace_id"] = serde_json::json!(Uuid::from_u128(9001));
            let changed: FileRecipientGrant = serde_json::from_value(g).unwrap();
            update_payload(&f, HEADS, 1, 1, &serde_json::to_vec(&changed).unwrap()).await;
            bad(&f).await;
            update_payload(&f, HEADS, 1, 1, &original).await;
            for (column, original_id, changed_id) in [
                (
                    "issuer_id",
                    account(1).principal().principal_id.as_uuid(),
                    account(8).principal().principal_id.as_uuid(),
                ),
                (
                    "issuer_account_id",
                    account(1).account_id().as_uuid(),
                    account(8).account_id().as_uuid(),
                ),
                (
                    "publisher_id",
                    account(2).principal().principal_id.as_uuid(),
                    account(8).principal().principal_id.as_uuid(),
                ),
                (
                    "publisher_account_id",
                    account(2).account_id().as_uuid(),
                    account(8).account_id().as_uuid(),
                ),
                (
                    "recipient_id",
                    account(3).principal().principal_id.as_uuid(),
                    account(8).principal().principal_id.as_uuid(),
                ),
                (
                    "recipient_account_id",
                    account(3).account_id().as_uuid(),
                    account(8).account_id().as_uuid(),
                ),
            ] {
                sql(&f, &format!("UPDATE {HEADS} SET {column}='{changed_id}'")).await;
                if column.starts_with("recipient_") {
                    assert_eq!(get(&f, 1, account(3)).await.unwrap(), None);
                    assert_eq!(
                        issue(&f, 9, account(9), account(9), account(9), validity()).await,
                        Err(FileRecipientGrantStoreError::InvalidRecord)
                    );
                } else {
                    bad(&f).await;
                }
                sql(&f, &format!("UPDATE {HEADS} SET {column}='{original_id}'")).await;
                assert_eq!(get(&f, 1, account(3)).await.unwrap(), Some(own.clone()));
            }
            for (column, value, restore) in [
                ("not_before", 101, 100),
                ("expires_at", 201, 200),
                ("revision", 2, 1),
            ] {
                sql(&f, &format!("UPDATE {HEADS} SET {column}={value}")).await;
                bad(&f).await;
                sql(&f, &format!("UPDATE {HEADS} SET {column}={restore}")).await;
            }
            let (name, definition) = remove_check(&f, HEADS, "octet_length").await;
            update_payload(&f, HEADS, 1, 1, &vec![b' '; MAX_FILE_GRANT_JSON_BYTES + 1]).await;
            bad(&f).await;
            update_payload(&f, HEADS, 1, 1, &original).await;
            restore_check(&f, HEADS, &name, &definition).await;
            assert_eq!(get(&f, 1, account(3)).await.unwrap(), Some(own));
        })
    })
    .await;
}
