use super::{
    bytes::{snapshot, tree},
    fixture::*,
};
use cyanrex_engine::services::{
    file_transfer::staging::ChunkStaging,
    file_upload_chunk_journal::{FileUploadChunkJournal, FileUploadChunkJournalError},
    file_upload_staging::{
        FileUploadStaging, JournaledFileUploadStaging, JournaledUploadChunkError, UploadChunkError,
    },
    file_upload_store::FileUploadStore,
};
use sqlx_postgres::PgPoolOptions;
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};
use uuid::Uuid;
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cyanrex-journal-default-{}", Uuid::new_v4()));
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

#[tokio::test]
async fn constructors_bound_entries_identity_namespace_and_adapter_scope_without_io() {
    let root = Root::new();
    let pool = lazy();
    let before = snapshot(&root.0);
    for entries in [0, 16_385, u32::MAX] {
        assert!(matches!(
            FileUploadChunkJournal::new(pool.clone(), scope(), Uuid::new_v4(), "_journal", entries),
            Err(FileUploadChunkJournalError::InvalidInput)
        ));
    }
    for entries in [1, 16_384] {
        assert!(FileUploadChunkJournal::new(
            pool.clone(),
            scope(),
            Uuid::new_v4(),
            "_journal",
            entries
        )
        .is_ok());
    }
    for installation in [Uuid::nil(), Uuid::from_u128(1)] {
        assert!(matches!(
            FileUploadChunkJournal::new(pool.clone(), scope(), installation, "_journal", 1),
            Err(FileUploadChunkJournalError::InvalidInput)
        ));
    }
    for name in [
        "",
        "public",
        "information_schema",
        "pg_private",
        "1bad",
        "Upper",
        "a.b",
        "a b",
        "a\";DROP",
        &"a".repeat(64),
    ] {
        assert!(matches!(
            FileUploadChunkJournal::new(pool.clone(), scope(), Uuid::new_v4(), name, 1),
            Err(FileUploadChunkJournalError::InvalidInput)
        ));
    }
    let source =
        FileUploadStore::new(pool.clone(), scope(), Uuid::new_v4(), "_source", limits()).unwrap();
    let local = FileUploadStaging::new(source, ChunkStaging::open(scope(), &root.0, 1).unwrap(), 4)
        .unwrap();
    let same =
        FileUploadChunkJournal::new(pool.clone(), scope(), Uuid::new_v4(), "_source", 1).unwrap();
    assert!(matches!(
        JournaledFileUploadStaging::new(local.clone(), same),
        Err(JournaledUploadChunkError::Local(
            UploadChunkError::InvalidInput
        ))
    ));
    for authority in [true, false] {
        let mut foreign = scope();
        if authority {
            foreign.authority_id = Uuid::from_u128(901).try_into().unwrap();
        } else {
            foreign.workspace_id = Uuid::from_u128(902).try_into().unwrap();
        }
        let journal =
            FileUploadChunkJournal::new(pool.clone(), foreign, Uuid::new_v4(), "_journal", 1)
                .unwrap();
        assert!(matches!(
            JournaledFileUploadStaging::new(local.clone(), journal),
            Err(JournaledUploadChunkError::Local(
                UploadChunkError::InvalidInput
            ))
        ));
    }
    assert_eq!(pool.size(), 0);
    assert_eq!(snapshot(&root.0), before);
    assert!(tree(&root.0).is_empty());
    pool.close().await;
}

#[tokio::test]
async fn closed_journal_and_cheap_get_rejections_never_touch_files_or_source() {
    let root = Root::new();
    let pool = lazy();
    let journal =
        FileUploadChunkJournal::new(pool.clone(), scope(), Uuid::new_v4(), "_journal", 1).unwrap();
    pool.close().await;
    let before = snapshot(&root.0);
    assert_eq!(
        journal.get(upload(1), owner(1), account(1), 0).await,
        Err(FileUploadChunkJournalError::StorageUnavailable)
    );
    assert_eq!(
        journal.get(upload(1), owner(1), account(1), 16_384).await,
        Err(FileUploadChunkJournalError::InvalidInput)
    );
    let mut foreign = owner(1);
    foreign.authority_id = Uuid::from_u128(901).try_into().unwrap();
    assert_eq!(
        journal.get(upload(1), foreign, account(1), 0).await,
        Err(FileUploadChunkJournalError::ScopeMismatch)
    );
    assert_eq!(snapshot(&root.0), before);
    assert!(tree(&root.0).is_empty());
}

#[tokio::test]
async fn dropping_unpolled_prepare_never_connects_or_creates_a_journal_or_chunk() {
    let root = Root::new();
    let source_pool = lazy();
    let journal_pool = lazy();
    let source = FileUploadStore::new(
        source_pool.clone(),
        scope(),
        Uuid::new_v4(),
        "_source",
        limits(),
    )
    .unwrap();
    let local = FileUploadStaging::new(source, ChunkStaging::open(scope(), &root.0, 1).unwrap(), 4)
        .unwrap();
    let journal =
        FileUploadChunkJournal::new(journal_pool.clone(), scope(), Uuid::new_v4(), "_journal", 1)
            .unwrap();
    let adapter = JournaledFileUploadStaging::new(local, journal).unwrap();
    let before = snapshot(&root.0);
    drop(adapter.prepare(upload(1), owner(1), account(1), 0));
    assert_eq!(source_pool.size(), 0);
    assert_eq!(journal_pool.size(), 0);
    assert_eq!(snapshot(&root.0), before);
    assert!(tree(&root.0).is_empty());
    source_pool.close().await;
    journal_pool.close().await;
}
