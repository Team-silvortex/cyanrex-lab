use super::*;
use cyanrex_engine::services::{
    file_transfer::staging::ChunkStaging,
    file_upload_chunk_journal::FileUploadChunkJournal,
    file_upload_publication::FileUploadPublicationStore,
    file_upload_staging::FileUploadStaging,
    file_upload_store::{FileUploadLimits, FileUploadStore},
};
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};

struct Closed {
    pool: PgPool,
    directory: PathBuf,
}
impl Closed {
    async fn new() -> Self {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://synthetic@127.0.0.1/unused")
            .unwrap();
        pool.close().await;
        let directory =
            std::env::temp_dir().join(format!("cyanrex-session-pub-config-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        Self { pool, directory }
    }
    fn publisher(&self, w: WorkspaceRef, names: [&str; 3]) -> FileUploadPublisher {
        let uploads = FileUploadStore::new(
            self.pool.clone(),
            w,
            Uuid::new_v4(),
            names[0],
            FileUploadLimits::new(0, 0, 2, 1).unwrap(),
        )
        .unwrap();
        let journal =
            FileUploadChunkJournal::new(self.pool.clone(), w, Uuid::new_v4(), names[1], 2).unwrap();
        let publications =
            FileUploadPublicationStore::new(self.pool.clone(), w, Uuid::new_v4(), names[2], 2)
                .unwrap();
        let staging = ChunkStaging::open(w, &self.directory, 1).unwrap();
        let local = FileUploadStaging::new(uploads, staging, 1).unwrap();
        FileUploadPublisher::new(local, journal, publications).unwrap()
    }
}
impl Drop for Closed {
    fn drop(&mut self) {
        fs::remove_dir(&self.directory)
            .expect("delete only the exclusive empty synthetic configuration directory");
    }
}

#[tokio::test]
async fn session_file_publishing_configuration_is_inert_with_closed_pools() {
    let f = Closed::new().await;
    let source = DurableAuthSource::new(f.pool.clone(), authority());
    let before = bytes::snapshot(&f.directory);
    let publisher = f.publisher(
        scope(),
        ["upload_intents", "confirmed_chunks", "publications"],
    );
    let grants = source
        .file_publisher_grant_workspace("publisher_grants", scope(), Uuid::new_v4(), 16)
        .unwrap();
    assert!(source.file_publishing_workspace(publisher, grants).is_ok());
    assert_eq!(bytes::snapshot(&f.directory), before);
    assert!(bytes::tree(&f.directory).is_empty());
}

#[tokio::test]
async fn session_file_publishing_configuration_pins_exact_authority_and_workspace() {
    let f = Closed::new().await;
    let source = DurableAuthSource::new(f.pool.clone(), authority());
    let mut foreign = scope();
    foreign.authority_id = id();
    let publisher = f.publisher(
        foreign,
        ["upload_intents", "confirmed_chunks", "publications"],
    );
    let grants = source
        .file_publisher_grant_workspace("publisher_grants", foreign, Uuid::new_v4(), 16)
        .unwrap();
    assert!(matches!(
        source.file_publishing_workspace(publisher, grants),
        Err(SessionFilePublishingError::Session(
            SessionCommandError::Authentication(DurableAuthError::SourceMismatch)
        ))
    ));
    let mut foreign = scope();
    foreign.workspace_id = id();
    let publisher = f.publisher(
        scope(),
        ["upload_intents", "confirmed_chunks", "publications"],
    );
    let grants = source
        .file_publisher_grant_workspace("publisher_grants", foreign, Uuid::new_v4(), 16)
        .unwrap();
    assert!(matches!(
        source.file_publishing_workspace(publisher, grants),
        Err(SessionFilePublishingError::Grant(
            FilePublisherGrantStoreError::ScopeMismatch
        ))
    ));
    assert!(bytes::tree(&f.directory).is_empty());
}

#[tokio::test]
async fn session_file_publishing_configuration_requires_four_distinct_safe_namespaces() {
    let f = Closed::new().await;
    let source = DurableAuthSource::new(f.pool.clone(), authority());
    for (names, grant_name) in [
        (
            ["upload_intents", "confirmed_chunks", "publications"],
            "upload_intents",
        ),
        (
            ["upload_intents", "confirmed_chunks", "publications"],
            "confirmed_chunks",
        ),
        (
            ["upload_intents", "confirmed_chunks", "publications"],
            "publications",
        ),
        (
            ["_private_uploads", "confirmed_chunks", "publications"],
            "publisher_grants",
        ),
    ] {
        let publisher = f.publisher(scope(), names);
        let grants = source
            .file_publisher_grant_workspace(grant_name, scope(), Uuid::new_v4(), 16)
            .unwrap();
        assert!(matches!(
            source.file_publishing_workspace(publisher, grants),
            Err(SessionFilePublishingError::InvalidNamespace)
        ));
    }
    assert!(bytes::tree(&f.directory).is_empty());
}
