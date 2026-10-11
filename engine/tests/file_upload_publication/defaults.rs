use super::cases::{bytes::*, fixture::*};
use cyanrex_engine::{
    models::collaboration::WorkspaceRef,
    services::{
        file_transfer::staging::ChunkStaging,
        file_upload_chunk_journal::FileUploadChunkJournal,
        file_upload_publication::{FileUploadPublicationStore, FileUploadPublicationStoreError},
        file_upload_staging::{FileUploadPublicationError, FileUploadPublisher, FileUploadStaging},
        file_upload_store::{FileUploadStore, FileUploadStoreError},
    },
};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};
use uuid::Uuid;

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-publication-default-{}", Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn lazy() -> PgPool {
    PgPoolOptions::new()
        .connect_lazy("postgres://fixture:fixture@127.0.0.1:1/unreachable")
        .unwrap()
}
fn source(pool: PgPool) -> FileUploadStore {
    FileUploadStore::new(pool, scope(), Uuid::new_v4(), "_source", limits()).unwrap()
}
fn journal(pool: PgPool, selected: WorkspaceRef, name: &str) -> FileUploadChunkJournal {
    FileUploadChunkJournal::new(pool, selected, Uuid::new_v4(), name, 16).unwrap()
}
fn publication(pool: PgPool, selected: WorkspaceRef, name: &str) -> FileUploadPublicationStore {
    FileUploadPublicationStore::new(pool, selected, Uuid::new_v4(), name, 8).unwrap()
}
fn local(pool: PgPool, root: &Root) -> FileUploadStaging {
    FileUploadStaging::new(
        source(pool),
        ChunkStaging::open(scope(), &root.0, 1).unwrap(),
        1,
    )
    .unwrap()
}

#[tokio::test]
async fn publication_handle_configuration_is_validated_without_storage_or_file_dispatch() {
    let pool = lazy();
    let root = Root::new();
    let before = snapshot(&root.0);
    for capacity in [0, 257, u32::MAX] {
        assert!(matches!(
            FileUploadPublicationStore::new(
                pool.clone(),
                scope(),
                Uuid::new_v4(),
                "_publication",
                capacity
            ),
            Err(FileUploadPublicationStoreError::InvalidInput)
        ));
    }
    for name in [
        "",
        "public",
        "pg_temp",
        "information_schema",
        "Upper",
        "two.names",
        "unsafe-name",
    ] {
        assert!(matches!(
            FileUploadPublicationStore::new(pool.clone(), scope(), Uuid::new_v4(), name, 8),
            Err(FileUploadPublicationStoreError::InvalidInput)
        ));
    }
    assert!(
        FileUploadPublicationStore::new(pool.clone(), scope(), Uuid::nil(), "_publication", 8)
            .is_err()
    );
    let local = local(pool.clone(), &root);
    let mut foreign = scope();
    foreign.workspace_id = Uuid::from_u128(900).try_into().unwrap();
    assert!(FileUploadPublisher::new(
        local.clone(),
        journal(pool.clone(), foreign, "_journal"),
        publication(pool.clone(), scope(), "_publication")
    )
    .is_err());
    assert!(FileUploadPublisher::new(
        local.clone(),
        journal(pool.clone(), scope(), "_journal"),
        publication(pool.clone(), foreign, "_publication")
    )
    .is_err());
    for (journal_name, publication_name) in [
        ("_source", "_publication"),
        ("_journal", "_source"),
        ("_journal", "_journal"),
    ] {
        assert!(FileUploadPublisher::new(
            local.clone(),
            journal(pool.clone(), scope(), journal_name),
            publication(pool.clone(), scope(), publication_name)
        )
        .is_err());
    }
    assert_eq!(pool.size(), 0);
    assert_eq!(snapshot(&root.0), before);
    assert!(tree(&root.0).is_empty());
    pool.close().await;
}

#[tokio::test]
async fn closed_source_and_foreign_authority_fail_before_current_file_io() {
    let pool = lazy();
    let root = Root::new();
    let publication = publication(pool.clone(), scope(), "_publication");
    let publisher = FileUploadPublisher::new(
        local(pool.clone(), &root),
        journal(pool.clone(), scope(), "_journal"),
        publication.clone(),
    )
    .unwrap();
    pool.close().await;
    let before = snapshot(&root.0);
    assert!(matches!(
        publisher.prepare(upload(1), owner(1), account(1)).await,
        Err(FileUploadPublicationError::Store(
            FileUploadStoreError::StorageUnavailable
        ))
    ));
    let mut foreign = owner(1);
    foreign.authority_id = Uuid::from_u128(900).try_into().unwrap();
    assert!(matches!(
        publisher.prepare(upload(1), foreign, account(1)).await,
        Err(FileUploadPublicationError::Store(
            FileUploadStoreError::ScopeMismatch
        ))
    ));
    assert_eq!(
        publication
            .get(upload(1), foreign, account(1))
            .await
            .unwrap_err(),
        FileUploadPublicationStoreError::ScopeMismatch
    );
    assert_eq!(snapshot(&root.0), before);
    assert!(tree(&root.0).is_empty());
}

#[tokio::test]
async fn unpolled_publication_prepare_and_metadata_read_have_no_side_effects() {
    let pool = lazy();
    let root = Root::new();
    let publication = publication(pool.clone(), scope(), "_publication");
    let publisher = FileUploadPublisher::new(
        local(pool.clone(), &root),
        journal(pool.clone(), scope(), "_journal"),
        publication.clone(),
    )
    .unwrap();
    let before = snapshot(&root.0);
    drop(publisher.prepare(upload(1), owner(1), account(1)));
    drop(publication.get(upload(1), owner(1), account(1)));
    assert_eq!(pool.size(), 0);
    assert_eq!(snapshot(&root.0), before);
    assert!(tree(&root.0).is_empty());
    pool.close().await;
}
