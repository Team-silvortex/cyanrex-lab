use super::{bytes::*, fixture::*};
use cyanrex_engine::services::{
    file_transfer::staging::ChunkStaging,
    file_upload_staging::{FileUploadStaging, UploadFileVerificationError},
    file_upload_store::{FileUploadStore, FileUploadStoreError},
};
use sqlx_postgres::PgPoolOptions;
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-whole-default-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn lazy() -> sqlx_postgres::PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://fixture:fixture@127.0.0.1:1/unreachable")
        .unwrap()
}
fn adapter(pool: sqlx_postgres::PgPool, root: &Root) -> FileUploadStaging {
    let source =
        FileUploadStore::new(pool, scope(), uuid::Uuid::new_v4(), "_source", limits()).unwrap();
    FileUploadStaging::new(source, ChunkStaging::open(scope(), &root.0, 1).unwrap(), 1).unwrap()
}

#[tokio::test]
async fn closed_source_reports_storage_failure_without_touching_current_files() {
    let root = Root::new();
    let pool = lazy();
    let adapter = adapter(pool.clone(), &root);
    pool.close().await;
    let before = snapshot(&root.0);
    let files = tree(&root.0);
    assert_eq!(
        adapter
            .verify_file(upload(1), owner(1), account(1))
            .await
            .unwrap_err(),
        UploadFileVerificationError::Store(FileUploadStoreError::StorageUnavailable)
    );
    assert_eq!(snapshot(&root.0), before);
    assert_eq!(tree(&root.0), files);
}
#[tokio::test]
async fn foreign_authority_is_rejected_before_closed_storage_or_file_io() {
    let root = Root::new();
    let pool = lazy();
    let adapter = adapter(pool.clone(), &root);
    pool.close().await;
    let before = snapshot(&root.0);
    let mut foreign = owner(1);
    foreign.authority_id = uuid::Uuid::from_u128(900).try_into().unwrap();
    assert_eq!(
        adapter
            .verify_file(upload(1), foreign, account(1))
            .await
            .unwrap_err(),
        UploadFileVerificationError::Store(FileUploadStoreError::ScopeMismatch)
    );
    assert_eq!(snapshot(&root.0), before);
    assert!(tree(&root.0).is_empty());
}
#[tokio::test]
async fn dropping_unpolled_whole_observation_never_connects_or_changes_files() {
    let root = Root::new();
    let pool = lazy();
    let adapter = adapter(pool.clone(), &root);
    let before = snapshot(&root.0);
    drop(adapter.verify_file(upload(1), owner(1), account(1)));
    assert_eq!(pool.size(), 0);
    assert_eq!(snapshot(&root.0), before);
    assert!(tree(&root.0).is_empty());
    pool.close().await;
}
