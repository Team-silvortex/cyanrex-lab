use crate::*;
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cyanrex-chunk-default-{}", Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn staging(&self, scope: WorkspaceRef) -> ChunkStaging {
        ChunkStaging::open(scope, &self.0, 1).unwrap()
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove only the exclusive synthetic default-test root");
    }
}
async fn closed() -> (DurableAuthSource, PgPool) {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    (DurableAuthSource::new(pool.clone(), authority()), pool)
}
fn metadata(
    source: &DurableAuthSource,
    pool: PgPool,
) -> cyanrex_engine::services::auth_service::durable_source::SessionFileDownloadWorkspace {
    let publications =
        FileUploadPublicationStore::new(pool, scope(), Uuid::new_v4(), "chunk_publications", 16)
            .unwrap();
    let recipients = source
        .file_recipient_grant_workspace("chunk_recipients", scope(), Uuid::new_v4(), 16)
        .unwrap();
    source
        .file_download_workspace(publications, recipients)
        .unwrap()
}
fn request() -> SessionFileChunkReadRequest {
    SessionFileChunkReadRequest {
        download: SessionFileDownloadRequest {
            grant_id: sharing_fixture::recipient_grant(1),
            upload_id: publication_fixture::upload(1),
            expected_file: FileVersionRef::from_manifest(&bytes::manifest(17, 1)),
        },
        chunk_index: 0,
    }
}

#[tokio::test]
async fn chunk_read_configuration_is_inert_with_bounded_capacity_and_exact_staging_scope() {
    let (source, pool) = closed().await;
    let root = Root::new();
    for capacity in [1, 16] {
        assert!(source
            .file_chunk_read_workspace(
                metadata(&source, pool.clone()),
                root.staging(scope()),
                capacity
            )
            .is_ok());
    }
    for capacity in [0, 17, usize::MAX] {
        assert_eq!(
            source
                .file_chunk_read_workspace(
                    metadata(&source, pool.clone()),
                    root.staging(scope()),
                    capacity
                )
                .err(),
            Some(SessionFileChunkReadError::InvalidInput)
        );
    }
    for authority_change in [true, false] {
        let mut foreign = scope();
        if authority_change {
            foreign.authority_id = id();
        } else {
            foreign.workspace_id = id();
        }
        assert_eq!(
            source
                .file_chunk_read_workspace(
                    metadata(&source, pool.clone()),
                    root.staging(foreign),
                    1
                )
                .err(),
            Some(SessionFileChunkReadError::InvalidInput)
        );
    }
}

#[tokio::test]
async fn chunk_read_invalid_token_global_index_and_foreign_file_scope_fail_before_storage() {
    let (source, pool) = closed().await;
    let root = Root::new();
    let workspace = source
        .file_chunk_read_workspace(metadata(&source, pool), root.staging(scope()), 1)
        .unwrap();
    let mut request = request();
    for token in ["", "invalid", "00000000-0000-0000-0000-000000000000"] {
        request.chunk_index = MAX_FILE_TRANSFER_CHUNKS as u32;
        assert_eq!(
            source
                .read_session_file_chunk(token, &workspace, &request)
                .await
                .err(),
            Some(SessionFileChunkReadError::Metadata(
                SessionFileDownloadError::Session(SessionCommandError::InvalidSession)
            ))
        );
    }
    for index in [MAX_FILE_TRANSFER_CHUNKS as u32, u32::MAX] {
        request.chunk_index = index;
        assert_eq!(
            source
                .read_session_file_chunk(&Uuid::new_v4().to_string(), &workspace, &request)
                .await
                .err(),
            Some(SessionFileChunkReadError::InvalidInput)
        );
    }
    request.chunk_index = 0;
    for field in ["authority_id", "workspace_id"] {
        let mut raw = serde_json::to_value(&request.download.expected_file).unwrap();
        raw["workspace"][field] = serde_json::json!(Uuid::new_v4());
        request.download.expected_file = serde_json::from_value(raw).unwrap();
        assert_eq!(
            source
                .read_session_file_chunk(&Uuid::new_v4().to_string(), &workspace, &request)
                .await
                .err(),
            Some(SessionFileChunkReadError::Metadata(
                SessionFileDownloadError::Recipient(FileRecipientGrantStoreError::ScopeMismatch)
            ))
        );
        request = self::request();
    }
}

#[tokio::test]
async fn chunk_read_valid_request_dispatches_to_explicit_closed_source() {
    let (source, pool) = closed().await;
    let root = Root::new();
    let workspace = source
        .file_chunk_read_workspace(metadata(&source, pool), root.staging(scope()), 1)
        .unwrap();
    assert_eq!(
        source
            .read_session_file_chunk(&Uuid::new_v4().to_string(), &workspace, &request())
            .await
            .err(),
        Some(SessionFileChunkReadError::Metadata(
            SessionFileDownloadError::Session(SessionCommandError::Authentication(
                DurableAuthError::StorageUnavailable
            ))
        ))
    );
}
