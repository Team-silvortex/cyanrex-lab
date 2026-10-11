use super::fixture::{limits, scope};
use cyanrex_engine::services::file_upload_store::{FileUploadStore, FileUploadStoreError};
use sqlx_postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn namespace_constructor_accepts_underscore_and_rejects_unsafe_names_without_io() {
    // Lazy pool: construction must not connect, install a schema, or inspect ambient databases.
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://fixture:fixture@127.0.0.1:1/unreachable")
        .unwrap();
    for name in ["_upload", "a", "file_upload_123", &"a".repeat(63)] {
        assert!(
            FileUploadStore::new(pool.clone(), scope(), Uuid::new_v4(), name, limits()).is_ok()
        );
    }
    for name in [
        "",
        "1upload",
        "Upload",
        "a-b",
        "a.b",
        "a b",
        "a\n",
        "public",
        "information_schema",
        "pg_private",
        "a\";DROP SCHEMA",
        "上传",
        &"a".repeat(64),
    ] {
        assert!(
            matches!(
                FileUploadStore::new(pool.clone(), scope(), Uuid::new_v4(), name, limits()),
                Err(FileUploadStoreError::InvalidInput)
            ),
            "unsafe namespace {name:?}"
        );
    }
    pool.close().await;
}
