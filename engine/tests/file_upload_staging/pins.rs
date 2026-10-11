use super::{
    bytes::{tree, Pattern},
    fixture::*,
};
use cyanrex_engine::services::file_upload_staging::{UploadChunkError, UploadChunkState};
use sqlx_core::query::query;
use std::sync::atomic::Ordering;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_prepared_schema_and_complete_source_record_drift_never_dispatches() {
    for drift in [
        "owner",
        "account",
        "manifest",
        "deleted",
        "installation",
        "relation",
    ] {
        let fixture = Fixture::ready(17).await;
        let original_oids = fixture.oids().await;
        let mut attempt = fixture
            .adapter
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        match drift {
            "owner" => {
                fixture
                    .sql(&format!(
                        "UPDATE {LEDGER} SET owner_id='{}'",
                        owner(2).principal_id
                    ))
                    .await
            }
            "account" => {
                fixture
                    .sql(&format!("UPDATE {LEDGER} SET account_id='{}'", account(2)))
                    .await
            }
            "manifest" => {
                let mut wire = serde_json::to_value(&fixture.manifest).unwrap();
                wire["display_name"] = serde_json::json!("other.bin");
                let changed =
                    cyanrex_engine::models::collaboration::FileTransferManifest::parse_json(
                        &serde_json::to_vec(&wire).unwrap(),
                    )
                    .unwrap();
                query(&format!("UPDATE {LEDGER} SET manifest=$1"))
                    .bind(serde_json::to_vec(&changed).unwrap())
                    .execute(&fixture.pool)
                    .await
                    .unwrap();
            }
            "deleted" => fixture.sql(&format!("DELETE FROM {LEDGER}")).await,
            "installation" => {
                fixture
                    .sql(&format!(
                        "UPDATE {MARKER} SET installation_id='{}'",
                        uuid::Uuid::new_v4()
                    ))
                    .await
            }
            "relation" => {
                fixture
                    .sql(&format!("ALTER TABLE {LEDGER} RENAME TO original_ledger"))
                    .await;
                fixture
                    .sql(&format!(
                        "CREATE TABLE {LEDGER}(LIKE original_ledger INCLUDING ALL)"
                    ))
                    .await;
                fixture
                    .sql(&format!(
                        "INSERT INTO {LEDGER} SELECT * FROM original_ledger"
                    ))
                    .await;
                fixture
                    .sql(&format!(
                        "UPDATE {MARKER} SET uploads_oid='{LEDGER}'::regclass::oid::bigint"
                    ))
                    .await;
                assert!(
                    fixture
                        .source
                        .get(upload(1), owner(1), account(1))
                        .await
                        .unwrap()
                        .is_some(),
                    "current schema is internally valid but not the original pin"
                );
            }
            _ => unreachable!(),
        }
        let before = fixture.rows().await;
        let oids = fixture.oids().await;
        let reader = Pattern::new(0, 17);
        let calls = reader.calls.clone();
        assert!(
            matches!(
                fixture.adapter.stage_chunk(&mut attempt, reader).await,
                Err(UploadChunkError::Store(_))
            ),
            "pre-write {drift}"
        );
        assert_eq!(attempt.state(), UploadChunkState::Unknown);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.rows().await, before);
        assert_eq!(fixture.oids().await, oids);
        assert!(tree(&fixture.root).is_empty());
        fixture.pool_restored().await;
        match drift {
            "owner" => {
                fixture
                    .sql(&format!(
                        "UPDATE {LEDGER} SET owner_id='{}'",
                        owner(1).principal_id
                    ))
                    .await
            }
            "account" => {
                fixture
                    .sql(&format!("UPDATE {LEDGER} SET account_id='{}'", account(1)))
                    .await
            }
            "manifest" => {
                query(&format!("UPDATE {LEDGER} SET manifest=$1"))
                    .bind(serde_json::to_vec(&fixture.manifest).unwrap())
                    .execute(&fixture.pool)
                    .await
                    .unwrap();
            }
            "deleted" => {
                query(&format!("INSERT INTO {LEDGER}(upload_id,owner_id,account_id,manifest) VALUES($1,$2,$3,$4)"))
                .bind(upload(1).as_uuid()).bind(owner(1).principal_id.as_uuid()).bind(account(1).as_uuid())
                .bind(serde_json::to_vec(&fixture.manifest).unwrap()).execute(&fixture.pool).await.unwrap();
            }
            "installation" => {
                fixture
                    .sql(&format!(
                        "UPDATE {MARKER} SET installation_id='{}'",
                        fixture.installation_id
                    ))
                    .await
            }
            "relation" => {
                fixture.sql(&format!("DROP TABLE {LEDGER}")).await;
                fixture
                    .sql(&format!("ALTER TABLE original_ledger RENAME TO {LEDGER}"))
                    .await;
                fixture
                    .sql(&format!(
                        "UPDATE {MARKER} SET uploads_oid={}",
                        original_oids[1]
                    ))
                    .await;
            }
            _ => unreachable!(),
        }
        let before = fixture.rows().await;
        let mut fresh = fixture
            .adapter
            .prepare(upload(1), owner(1), account(1), 0)
            .await
            .unwrap();
        fixture
            .adapter
            .stage_chunk(&mut fresh, Pattern::new(0, 17))
            .await
            .unwrap();
        assert_eq!(fresh.state(), UploadChunkState::Confirmed);
        assert_eq!(attempt.state(), UploadChunkState::Unknown);
        assert_eq!(fixture.rows().await, before);
        assert_eq!(fixture.oids().await, original_oids);
        fixture.cleanup().await;
    }
    let healthy = Fixture::ready(17).await;
    let before = healthy.rows().await;
    let mut ready = healthy
        .adapter
        .prepare(upload(1), owner(1), account(1), 0)
        .await
        .unwrap();
    healthy
        .adapter
        .stage_chunk(&mut ready, Pattern::new(0, 17))
        .await
        .unwrap();
    assert_eq!(ready.state(), UploadChunkState::Confirmed);
    assert_eq!(healthy.rows().await, before);
    healthy.cleanup().await;
}
