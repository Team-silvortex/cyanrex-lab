use super::{bytes::tree, fixture::*};
use cyanrex_engine::services::{
    file_transfer::staging::ChunkStaging,
    file_upload_staging::{FileUploadStaging, UploadChunkError},
    file_upload_store::{FileUploadStore, FileUploadStoreError},
};
use sqlx_postgres::PgPoolOptions;
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-upload-default-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn source(pool: sqlx_postgres::PgPool) -> FileUploadStore {
    FileUploadStore::new(
        pool,
        scope(),
        uuid::Uuid::new_v4(),
        "_upload_test",
        limits(),
    )
    .unwrap()
}
fn lazy() -> sqlx_postgres::PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://fixture:fixture@127.0.0.1:1/unreachable")
        .unwrap()
}

#[tokio::test]
async fn constructor_bounds_controls_and_scope_without_installing_or_modifying_roots() {
    let root = Root::new();
    let pool = lazy();
    let store = source(pool.clone());
    let staging = ChunkStaging::open(scope(), &root.0, 1).unwrap();
    let before = super::bytes::snapshot(&root.0);
    for controls in [0, 17, usize::MAX] {
        assert!(matches!(
            FileUploadStaging::new(store.clone(), staging.clone(), controls),
            Err(UploadChunkError::InvalidInput)
        ));
    }
    for controls in [1, 16] {
        assert!(FileUploadStaging::new(store.clone(), staging.clone(), controls).is_ok());
    }
    for authority in [false, true] {
        let mut other = scope();
        if authority {
            other.authority_id = uuid::Uuid::from_u128(901).try_into().unwrap();
        } else {
            other.workspace_id = uuid::Uuid::from_u128(902).try_into().unwrap();
        }
        let foreign = ChunkStaging::open(other, &root.0, 1).unwrap();
        assert!(matches!(
            FileUploadStaging::new(store.clone(), foreign, 4),
            Err(UploadChunkError::InvalidInput)
        ));
    }
    assert_eq!(super::bytes::snapshot(&root.0), before);
    assert!(tree(&root.0).is_empty());
    pool.close().await;
}

#[tokio::test]
async fn closed_source_prepare_rejects_without_creating_or_chmodding_entries() {
    let root = Root::new();
    let pool = lazy();
    let store = source(pool.clone());
    pool.close().await;
    let adapter =
        FileUploadStaging::new(store, ChunkStaging::open(scope(), &root.0, 1).unwrap(), 4).unwrap();
    let before = super::bytes::snapshot(&root.0);
    assert!(matches!(
        adapter.prepare(upload(1), owner(1), account(1), 0).await,
        Err(UploadChunkError::Store(
            FileUploadStoreError::StorageUnavailable
        ))
    ));
    assert_eq!(super::bytes::snapshot(&root.0), before);
    assert!(tree(&root.0).is_empty());
}

#[tokio::test]
async fn dropping_unpolled_prepare_never_connects_or_touches_the_root() {
    let root = Root::new();
    let pool = lazy();
    let store = source(pool.clone());
    let adapter =
        FileUploadStaging::new(store, ChunkStaging::open(scope(), &root.0, 1).unwrap(), 1).unwrap();
    let before = super::bytes::snapshot(&root.0);
    drop(adapter.prepare(upload(1), owner(1), account(1), 0));
    assert_eq!(pool.size(), 0);
    assert_eq!(super::bytes::snapshot(&root.0), before);
    assert!(tree(&root.0).is_empty());
    pool.close().await;
}
