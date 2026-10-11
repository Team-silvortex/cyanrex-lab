use super::fixture::*;
use cyanrex_engine::services::file_upload_store::FileUploadStoreError;
use serde_json::json;

fn encoded(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_postwrite_foreign_mutations_and_unexpected_deltas_never_confirm() {
    for deferred in [false, true] {
        for mutation in [
            "foreign_manifest",
            "foreign_owner",
            "foreign_account",
            "delete_foreign",
            "extra_row",
            "new_manifest",
            "new_owner",
            "new_account",
        ] {
            let fixture = Fixture::ready(limits()).await;
            let original = manifest(11, 1);
            let foreign = manifest(11, 2);
            fixture
                .store
                .reserve(upload(1), owner(1), account(1), &original)
                .await
                .unwrap();
            fixture
                .store
                .reserve(upload(2), owner(2), account(2), &foreign)
                .await
                .unwrap();
            let altered_foreign = changed(&foreign, "display_name", json!("另一份.bin"));
            let altered_candidate = changed(&manifest(1, 3), "display_name", json!("another.bin"));
            let body = match mutation {
                "foreign_manifest" => format!("UPDATE {LEDGER} SET manifest=decode('{}','hex') WHERE upload_id='{}';",
                    encoded(&serde_json::to_vec(&altered_foreign).unwrap()), upload(2)),
                "foreign_owner" => format!("UPDATE {LEDGER} SET owner_id='{}' WHERE upload_id='{}';", owner(9).principal_id, upload(2)),
                "foreign_account" => format!("UPDATE {LEDGER} SET account_id='{}' WHERE upload_id='{}';", account(9), upload(2)),
                "delete_foreign" => format!("DELETE FROM {LEDGER} WHERE upload_id='{}';", upload(2)),
                "extra_row" => format!("IF NEW.upload_id='{}' THEN INSERT INTO {LEDGER}(upload_id,owner_id,account_id,manifest)
                    VALUES('{}','{}','{}',decode('{}','hex')); END IF;", upload(3), upload(9), owner(9).principal_id,
                    account(9), encoded(&serde_json::to_vec(&manifest(0, 9)).unwrap())),
                "new_manifest" => format!("UPDATE {LEDGER} SET manifest=decode('{}','hex') WHERE upload_id=NEW.upload_id;",
                    encoded(&serde_json::to_vec(&altered_candidate).unwrap())),
                "new_owner" => format!("UPDATE {LEDGER} SET owner_id='{}' WHERE upload_id=NEW.upload_id;", owner(9).principal_id),
                "new_account" => format!("UPDATE {LEDGER} SET account_id='{}' WHERE upload_id=NEW.upload_id;", account(9)),
                _ => unreachable!(),
            };
            fixture.fault(&body, deferred, true).await;
            let before = fixture.rows().await;
            let oids = fixture.oids().await;
            assert_eq!(
                fixture
                    .store
                    .reserve(upload(3), owner(1), account(3), &manifest(1, 3))
                    .await,
                Err(FileUploadStoreError::InvalidRecord),
                "{mutation}, deferred={deferred}"
            );
            assert!(
                fixture.hit().await,
                "post-insert fault must actually execute"
            );
            assert_eq!(
                fixture.rows().await,
                before,
                "controlled precommit fault must roll back the entire ledger"
            );
            assert_eq!(fixture.oids().await, oids);
            fixture.pool_restored().await;
            fixture.remove_fault().await;
            fixture
                .store
                .reserve(upload(3), owner(1), account(3), &manifest(1, 3))
                .await
                .unwrap();
            let before = fixture.rows().await;
            assert_eq!(
                fixture
                    .store
                    .get(upload(1), owner(1), account(1))
                    .await
                    .unwrap()
                    .unwrap()
                    .manifest(),
                &original
            );
            assert_eq!(
                fixture
                    .store
                    .get(upload(2), owner(2), account(2))
                    .await
                    .unwrap()
                    .unwrap()
                    .manifest(),
                &foreign
            );
            assert_eq!(fixture.rows().await, before);
            fixture.cleanup().await;
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_insert_and_deferred_commit_faults_preserve_prior_reservations() {
    for failure in [
        "suppressed",
        "insert_error",
        "deferred_error",
        "marker_drift",
        "search_path_drift",
    ] {
        let fixture = Fixture::ready(quota_limits(12, 12, 3, 3)).await;
        fixture
            .store
            .reserve(upload(1), owner(1), account(1), &manifest(4, 1))
            .await
            .unwrap();
        let (body, deferred, after, error) = match failure {
            "suppressed" => (
                "RETURN NULL;".to_owned(),
                false,
                false,
                FileUploadStoreError::StorageUnavailable,
            ),
            "insert_error" => (
                "RAISE EXCEPTION 'controlled insert rejection';".to_owned(),
                false,
                false,
                FileUploadStoreError::StorageUnavailable,
            ),
            "deferred_error" => (
                "RAISE EXCEPTION 'controlled deferred rejection';".to_owned(),
                true,
                true,
                FileUploadStoreError::StorageUnavailable,
            ),
            "marker_drift" => (
                format!("UPDATE {MARKER} SET total_bytes=11,owner_bytes=11;"),
                true,
                true,
                FileUploadStoreError::UnsupportedSchema,
            ),
            "search_path_drift" => (
                format!(
                    "PERFORM set_config('search_path','{},pg_catalog',true);",
                    fixture.schema
                ),
                true,
                true,
                FileUploadStoreError::UnsupportedSchema,
            ),
            _ => unreachable!(),
        };
        fixture.fault(&body, deferred, after).await;
        let before = fixture.rows().await;
        let oids = fixture.oids().await;
        assert_eq!(
            fixture
                .store
                .reserve(upload(2), owner(1), account(2), &manifest(8, 2))
                .await,
            Err(error),
            "{failure}"
        );
        assert!(fixture.hit().await);
        fixture.drain().await;
        assert_eq!(fixture.rows().await, before);
        assert_eq!(fixture.oids().await, oids);
        fixture.pool_restored().await;
        fixture.remove_fault().await;
        fixture
            .store
            .reserve(upload(2), owner(1), account(2), &manifest(8, 2))
            .await
            .unwrap();
        let before = fixture.rows().await;
        assert!(fixture
            .store
            .get(upload(1), owner(1), account(1))
            .await
            .unwrap()
            .is_some());
        assert!(fixture
            .store
            .get(upload(2), owner(1), account(2))
            .await
            .unwrap()
            .is_some());
        assert_eq!(fixture.rows().await, before);
        fixture.cleanup().await;
    }
    // These injected failures happen before COMMIT; no claim is made about lost commit acknowledgements.
}
