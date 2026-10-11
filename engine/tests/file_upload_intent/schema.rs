use super::fixture::*;
use cyanrex_engine::services::file_upload_store::{FileUploadStore, FileUploadStoreError};
use sqlx_core::{query::query, row::Row};
use uuid::Uuid;

async fn rejected_without_changes(
    fixture: &Fixture,
    store: &FileUploadStore,
    error: FileUploadStoreError,
) {
    let before = fixture.rows().await;
    let oids = fixture.oids().await;
    assert_eq!(store.get(upload(1), owner(1), account(1)).await, Err(error));
    assert_eq!(
        store
            .reserve(upload(2), owner(1), account(2), &manifest(1, 2))
            .await,
        Err(error)
    );
    assert_eq!(fixture.rows().await, before);
    assert_eq!(fixture.oids().await, oids);
    fixture.pool_restored().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_installation_limits_scope_and_schema_shape_pins_are_not_adopted() {
    let occupied = Fixture::new(limits()).await;
    occupied
        .sql("CREATE TABLE unrelated(value bytea NOT NULL)")
        .await;
    occupied
        .sql("INSERT INTO unrelated VALUES(decode('aabbcc','hex'))")
        .await;
    let before: String = query("SELECT row_to_json(t)::text AS row FROM unrelated t")
        .fetch_one(&occupied.pool)
        .await
        .unwrap()
        .get("row");
    assert_eq!(
        occupied.store.install_empty_namespace().await,
        Err(FileUploadStoreError::NamespaceNotEmpty)
    );
    let after: String = query("SELECT row_to_json(t)::text AS row FROM unrelated t")
        .fetch_one(&occupied.pool)
        .await
        .unwrap()
        .get("row");
    assert_eq!(before, after);
    let count: i64 =
        query("SELECT count(*) AS n FROM pg_class WHERE relnamespace=to_regnamespace($1)")
            .bind(&occupied.schema)
            .fetch_one(&occupied.admin)
            .await
            .unwrap()
            .get("n");
    assert_eq!(
        count, 1,
        "failed installation cannot create incidental upload relations"
    );
    occupied.cleanup().await;

    let fixture = Fixture::ready(limits()).await;
    fixture
        .store
        .reserve(upload(1), owner(1), account(1), &manifest(11, 1))
        .await
        .unwrap();
    let before = fixture.rows().await;
    assert_eq!(
        fixture.store.install_empty_namespace().await,
        Err(FileUploadStoreError::NamespaceNotEmpty)
    );
    assert_eq!(fixture.rows().await, before);
    let wrong_install = FileUploadStore::new(
        fixture.pool.clone(),
        scope(),
        Uuid::new_v4(),
        &fixture.schema,
        limits(),
    )
    .unwrap();
    rejected_without_changes(
        &fixture,
        &wrong_install,
        FileUploadStoreError::IncarnationMismatch,
    )
    .await;
    let wrong_limits = FileUploadStore::new(
        fixture.pool.clone(),
        scope(),
        fixture.installation_id,
        &fixture.schema,
        quota_limits(1023, 511, 15, 7),
    )
    .unwrap();
    rejected_without_changes(
        &fixture,
        &wrong_limits,
        FileUploadStoreError::UnsupportedSchema,
    )
    .await;
    for authority in [true, false] {
        let mut other = scope();
        if authority {
            other.authority_id = Uuid::from_u128(9001).try_into().unwrap();
        } else {
            other.workspace_id = Uuid::from_u128(9002).try_into().unwrap();
        }
        let other_store = FileUploadStore::new(
            fixture.pool.clone(),
            other,
            fixture.installation_id,
            &fixture.schema,
            limits(),
        )
        .unwrap();
        let before = fixture.rows().await;
        let mut actor = owner(1);
        actor.authority_id = other.authority_id;
        assert_eq!(
            other_store.get(upload(1), actor, account(1)).await,
            Err(FileUploadStoreError::ScopeMismatch)
        );
        assert_eq!(fixture.rows().await, before);
    }

    for column in ["namespace_oid", "schema_oid", "uploads_oid"] {
        let original: i64 = query(&format!("SELECT {column} AS value FROM {MARKER}"))
            .fetch_one(&fixture.pool)
            .await
            .unwrap()
            .get("value");
        fixture
            .sql(&format!("UPDATE {MARKER} SET {column}={}", original + 1))
            .await;
        rejected_without_changes(
            &fixture,
            &fixture.store,
            FileUploadStoreError::UnsupportedSchema,
        )
        .await;
        fixture
            .sql(&format!("UPDATE {MARKER} SET {column}={original}"))
            .await;
        assert!(fixture
            .store
            .get(upload(1), owner(1), account(1))
            .await
            .unwrap()
            .is_some());
    }
    let checks = fixture.drop_constraints(MARKER, "version", "c").await;
    fixture
        .sql(&format!("UPDATE {MARKER} SET version=99"))
        .await;
    rejected_without_changes(
        &fixture,
        &fixture.store,
        FileUploadStoreError::UnsupportedSchema,
    )
    .await;
    fixture.sql(&format!("UPDATE {MARKER} SET version=1")).await;
    fixture.restore_constraints(MARKER, checks).await;

    for column in ["owner_bytes", "total_uploads"] {
        let checks = fixture.drop_constraints(MARKER, column, "c").await;
        let original: i64 = query(&format!("SELECT {column}::bigint AS value FROM {MARKER}"))
            .fetch_one(&fixture.pool)
            .await
            .unwrap()
            .get("value");
        fixture
            .sql(&format!("UPDATE {MARKER} SET {column}=-1"))
            .await;
        rejected_without_changes(
            &fixture,
            &fixture.store,
            FileUploadStoreError::InvalidRecord,
        )
        .await;
        fixture
            .sql(&format!("UPDATE {MARKER} SET {column}={original}"))
            .await;
        fixture.restore_constraints(MARKER, checks).await;
    }
    let primary = fixture.drop_constraints(LEDGER, "upload_id", "p").await;
    rejected_without_changes(
        &fixture,
        &fixture.store,
        FileUploadStoreError::UnsupportedSchema,
    )
    .await;
    fixture.restore_constraints(LEDGER, primary).await;
    fixture
        .sql(&format!(
            "ALTER TABLE {LEDGER} ALTER COLUMN account_id DROP NOT NULL"
        ))
        .await;
    rejected_without_changes(
        &fixture,
        &fixture.store,
        FileUploadStoreError::UnsupportedSchema,
    )
    .await;
    fixture
        .sql(&format!(
            "ALTER TABLE {LEDGER} ALTER COLUMN account_id SET NOT NULL"
        ))
        .await;
    fixture
        .sql(&format!(
            "ALTER TABLE {LEDGER} ALTER COLUMN account_id TYPE text USING account_id::text"
        ))
        .await;
    rejected_without_changes(
        &fixture,
        &fixture.store,
        FileUploadStoreError::UnsupportedSchema,
    )
    .await;
    fixture
        .sql(&format!(
            "ALTER TABLE {LEDGER} ALTER COLUMN account_id TYPE uuid USING account_id::uuid"
        ))
        .await;
    fixture
        .sql(&format!("ALTER TABLE {LEDGER} ENABLE ROW LEVEL SECURITY"))
        .await;
    rejected_without_changes(
        &fixture,
        &fixture.store,
        FileUploadStoreError::UnsupportedSchema,
    )
    .await;
    fixture
        .sql(&format!("ALTER TABLE {LEDGER} DISABLE ROW LEVEL SECURITY"))
        .await;
    fixture
        .sql(&format!(
            "ALTER TABLE {LEDGER} ADD COLUMN incidental integer"
        ))
        .await;
    rejected_without_changes(
        &fixture,
        &fixture.store,
        FileUploadStoreError::UnsupportedSchema,
    )
    .await;
    fixture
        .sql(&format!("ALTER TABLE {LEDGER} DROP COLUMN incidental"))
        .await;
    fixture
        .store
        .reserve(upload(2), owner(1), account(2), &manifest(1, 2))
        .await
        .unwrap();
    let before = fixture.rows().await;
    assert!(fixture
        .store
        .get(upload(1), owner(1), account(1))
        .await
        .unwrap()
        .is_some());
    assert_eq!(fixture.rows().await, before);
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_query_shadow_and_in_transaction_relation_replacement_are_rejected() {
    let fixture = Fixture::ready(limits()).await;
    fixture
        .store
        .reserve(upload(1), owner(1), account(1), &manifest(11, 1))
        .await
        .unwrap();
    for shadow in ["temp", "search_path", "wrong_namespace"] {
        let selected = pool_for_from_fixture(&fixture).await;
        let name = if shadow == "wrong_namespace" {
            "other_expected_namespace"
        } else {
            &fixture.schema
        };
        let store = FileUploadStore::new(
            selected.clone(),
            scope(),
            fixture.installation_id,
            name,
            limits(),
        )
        .unwrap();
        match shadow {
            "temp" => {
                query(&format!(
                    "CREATE TEMP TABLE {LEDGER}(LIKE {}.{LEDGER} INCLUDING ALL)",
                    fixture.schema
                ))
                .execute(&selected)
                .await
                .unwrap();
            }
            "search_path" => {
                query("SELECT set_config('search_path',$1,false)")
                    .bind(format!("{},public", fixture.schema))
                    .execute(&selected)
                    .await
                    .unwrap();
            }
            _ => {}
        }
        rejected_without_changes(&fixture, &store, FileUploadStoreError::UnsupportedSchema).await;
        selected.close().await;
    }
    for relation in [MARKER, LEDGER] {
        let before = fixture.rows().await;
        let oids = fixture.oids().await;
        let body = format!(
            "ALTER TABLE {relation} RENAME TO replaced_upload_relation;
            CREATE TABLE {relation}(LIKE replaced_upload_relation INCLUDING ALL);
            INSERT INTO {relation} SELECT * FROM replaced_upload_relation;"
        );
        fixture.fault(&body, false, true).await;
        assert_eq!(
            fixture
                .store
                .reserve(upload(2), owner(1), account(2), &manifest(1, 2))
                .await,
            Err(FileUploadStoreError::UnsupportedSchema),
            "same bytes and schema must not adopt a different relation OID"
        );
        assert!(fixture.hit().await, "replacement trigger must really run");
        assert_eq!(fixture.rows().await, before);
        assert_eq!(fixture.oids().await, oids);
        fixture.remove_fault().await;
        let restored = fixture.rows().await;
        assert!(fixture
            .store
            .get(upload(1), owner(1), account(1))
            .await
            .unwrap()
            .is_some());
        assert_eq!(fixture.rows().await, restored);
    }
    // External same-shape/same-content replacement cannot be silently adopted on the next call.
    fixture
        .sql(&format!(
            "ALTER TABLE {LEDGER} RENAME TO original_upload_relation"
        ))
        .await;
    fixture
        .sql(&format!(
            "CREATE TABLE {LEDGER}(LIKE original_upload_relation INCLUDING ALL)"
        ))
        .await;
    fixture
        .sql(&format!(
            "INSERT INTO {LEDGER} SELECT * FROM original_upload_relation"
        ))
        .await;
    rejected_without_changes(
        &fixture,
        &fixture.store,
        FileUploadStoreError::UnsupportedSchema,
    )
    .await;
    fixture.sql(&format!("DROP TABLE {LEDGER}")).await;
    fixture
        .sql(&format!(
            "ALTER TABLE original_upload_relation RENAME TO {LEDGER}"
        ))
        .await;
    fixture
        .store
        .reserve(upload(2), owner(1), account(2), &manifest(1, 2))
        .await
        .unwrap();
    fixture.pool_restored().await;
    fixture.cleanup().await;
}

async fn pool_for_from_fixture(fixture: &Fixture) -> sqlx_postgres::PgPool {
    // Explicit fixture URL only; this single-connection pool makes a real temporary shadow stable.
    let url =
        std::env::var("CYANREX_TEST_DATABASE_URL").expect("explicit disposable database required");
    pool_for(&url, &fixture.schema, 1).await
}
