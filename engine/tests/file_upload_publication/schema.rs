use super::{fixture::*, records, support::*};
use cyanrex_engine::services::{
    file_upload_publication::{FileUploadPublicationStore, FileUploadPublicationStoreError},
    file_upload_staging::{FileUploadPublicationError, FileUploadPublicationState},
};
use sqlx_core::{query::query, row::Row};

pub async fn checks(f: &Fixture) -> Vec<(String, String)> {
    let rows=query("SELECT conname,pg_get_constraintdef(oid) AS definition FROM pg_constraint WHERE conrelid=to_regclass($1) AND contype='c'")
        .bind(format!("{}.{PUBLICATION_ROWS}",f.publication_schema)).fetch_all(&f.publication_pool).await.unwrap();
    let mut saved = Vec::new();
    for row in rows {
        let name: String = row.get("conname");
        let definition: String = row.get("definition");
        assert!(name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'));
        f.sql(
            Store::Publication,
            &format!("ALTER TABLE {PUBLICATION_ROWS} DROP CONSTRAINT {name}"),
        )
        .await;
        saved.push((name, definition));
    }
    assert!(!saved.is_empty());
    saved
}
pub async fn restore(f: &Fixture, saved: Vec<(String, String)>) {
    for (name, definition) in saved {
        f.sql(
            Store::Publication,
            &format!("ALTER TABLE {PUBLICATION_ROWS} ADD CONSTRAINT {name} {definition}"),
        )
        .await;
    }
}
async fn bad_get(
    f: &Fixture,
    store: &FileUploadPublicationStore,
    error: FileUploadPublicationStoreError,
) {
    let before = snapshot_all(f).await;
    assert_eq!(store.get(upload(1), owner(1), account(1)).await, Err(error));
    assert_eq!(snapshot_all(f).await, before);
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_original_publication_pins_shadow_and_corrupt_foreign_records_fail_closed() {
    let occupied = Fixture::new(4, 2, 16, 8).await;
    occupied
        .sql(
            Store::Publication,
            "CREATE TABLE unrelated(value bytea NOT NULL)",
        )
        .await;
    occupied
        .sql(
            Store::Publication,
            "INSERT INTO unrelated VALUES(decode('aabb','hex'))",
        )
        .await;
    let before: String = query("SELECT row_to_json(t)::text AS v FROM unrelated t")
        .fetch_one(&occupied.publication_pool)
        .await
        .unwrap()
        .get("v");
    assert_eq!(
        occupied.publication.install_empty_namespace().await,
        Err(FileUploadPublicationStoreError::NamespaceNotEmpty)
    );
    assert_eq!(
        query("SELECT row_to_json(t)::text AS v FROM unrelated t")
            .fetch_one(&occupied.publication_pool)
            .await
            .unwrap()
            .get::<String, _>("v"),
        before
    );
    assert_eq!(
        query("SELECT count(*) AS n FROM pg_class WHERE relnamespace=to_regnamespace($1)")
            .bind(&occupied.publication_schema)
            .fetch_one(&occupied.publication_admin)
            .await
            .unwrap()
            .get::<i64, _>("n"),
        1
    );
    occupied.cleanup().await;

    let mut f = Fixture::ready(17).await;
    stage_all(&f).await;
    let installed = snapshot_all(&f).await;
    assert_eq!(
        f.publication.install_empty_namespace().await,
        Err(FileUploadPublicationStoreError::NamespaceNotEmpty)
    );
    assert_eq!(snapshot_all(&f).await, installed);
    for (id, max, error) in [
        (
            uuid::Uuid::new_v4(),
            8,
            FileUploadPublicationStoreError::IncarnationMismatch,
        ),
        (
            f.publication_installation_id,
            7,
            FileUploadPublicationStoreError::UnsupportedSchema,
        ),
    ] {
        let store = FileUploadPublicationStore::new(
            f.publication_pool.clone(),
            scope(),
            id,
            &f.publication_schema,
            max,
        )
        .unwrap();
        bad_get(&f, &store, error).await;
    }
    for column in ["namespace_oid", "schema_oid", "publications_oid"] {
        let original: i64 = query(&format!("SELECT {column} AS v FROM {PUBLICATION_MARKER}"))
            .fetch_one(&f.publication_pool)
            .await
            .unwrap()
            .get("v");
        f.sql(
            Store::Publication,
            &format!("UPDATE {PUBLICATION_MARKER} SET {column}={}", original + 1),
        )
        .await;
        bad_get(
            &f,
            &f.publication,
            FileUploadPublicationStoreError::UnsupportedSchema,
        )
        .await;
        f.sql(
            Store::Publication,
            &format!("UPDATE {PUBLICATION_MARKER} SET {column}={original}"),
        )
        .await;
        assert_eq!(snapshot_all(&f).await, installed);
    }
    for (bad,good) in [
        (format!("ALTER TABLE {PUBLICATION_ROWS} ALTER COLUMN account_id DROP NOT NULL"),format!("ALTER TABLE {PUBLICATION_ROWS} ALTER COLUMN account_id SET NOT NULL")),
        (format!("ALTER TABLE {PUBLICATION_ROWS} ALTER COLUMN account_id TYPE text USING account_id::text"),format!("ALTER TABLE {PUBLICATION_ROWS} ALTER COLUMN account_id TYPE uuid USING account_id::uuid")),
        (format!("ALTER TABLE {PUBLICATION_ROWS} ENABLE ROW LEVEL SECURITY"),format!("ALTER TABLE {PUBLICATION_ROWS} DISABLE ROW LEVEL SECURITY")),
    ] {f.sql(Store::Publication,&bad).await;bad_get(&f,&f.publication,FileUploadPublicationStoreError::UnsupportedSchema).await;f.sql(Store::Publication,&good).await;}
    let mut original = f
        .publisher
        .prepare(upload(1), owner(1), account(1))
        .await
        .unwrap();
    for statement in [format!("ALTER TABLE {PUBLICATION_ROWS} RENAME TO original_publications"),
        format!("CREATE TABLE {PUBLICATION_ROWS}(LIKE original_publications INCLUDING ALL)"),
        format!("UPDATE {PUBLICATION_MARKER} SET publications_oid='{PUBLICATION_ROWS}'::regclass::oid::bigint")] {
        f.sql(Store::Publication,&statement).await;
    }
    let replacement = snapshot_all(&f).await;
    assert_eq!(
        f.publisher.publish(&mut original).await.unwrap_err(),
        FileUploadPublicationError::Publication(FileUploadPublicationStoreError::UnsupportedSchema)
    );
    assert_eq!(original.state(), FileUploadPublicationState::Unknown);
    assert_eq!(snapshot_all(&f).await, replacement);
    f.sql(
        Store::Publication,
        &format!("DROP TABLE {PUBLICATION_ROWS}"),
    )
    .await;
    f.sql(
        Store::Publication,
        &format!("ALTER TABLE original_publications RENAME TO {PUBLICATION_ROWS}"),
    )
    .await;
    f.sql(Store::Publication,&format!("UPDATE {PUBLICATION_MARKER} SET publications_oid='{PUBLICATION_ROWS}'::regclass::oid::bigint")).await;
    assert_eq!(snapshot_all(&f).await, installed);

    // All pooled sessions have real pg_temp shadows, while current_schema remains the expected one.
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.publication_pool.acquire().await.unwrap();
        for table in [PUBLICATION_MARKER, PUBLICATION_ROWS] {
            query(&format!(
                "CREATE TEMP TABLE {table}(LIKE {}.{table} INCLUDING ALL)",
                f.publication_schema
            ))
            .execute(&mut *c)
            .await
            .unwrap();
        }
        held.push(c);
    }
    drop(held);
    bad_get(
        &f,
        &f.publication,
        FileUploadPublicationStoreError::UnsupportedSchema,
    )
    .await;
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.publication_pool.acquire().await.unwrap();
        query(&format!(
            "DROP TABLE pg_temp.{PUBLICATION_MARKER},pg_temp.{PUBLICATION_ROWS}"
        ))
        .execute(&mut *c)
        .await
        .unwrap();
        held.push(c);
    }
    drop(held);
    // pg_my_temp_schema() remains nonzero for these backends even after DROP. Only fresh sessions
    // restore this boundary; dropping the shadows is not asserted to make the old pool healthy.
    f.publication_pool.close().await;
    let (store, pool) = reopen(&f).await;
    f.publication_pool = pool;
    f.publication = store;
    f.publisher = cyanrex_engine::services::file_upload_staging::FileUploadPublisher::new(
        f.local.clone(),
        f.journal.clone(),
        f.publication.clone(),
    )
    .unwrap();
    publish(&f, &f.publisher, 1, &f.manifest).await;
    f.cleanup().await;
    records::exercise().await;
}
