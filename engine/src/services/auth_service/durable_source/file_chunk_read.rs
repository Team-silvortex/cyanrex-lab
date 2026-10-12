//! Current Session-authorized verified chunk snapshot, not a transport or durable receipt.
use super::*;
use crate::{
    models::collaboration::{
        FileAccountRef, FileGrantState, FileVersionRef, Sha256Digest, WorkspaceRef,
        MAX_FILE_TRANSFER_CHUNKS,
    },
    services::{
        file_recipient_grant_store::FileRecipientGrantStoreError,
        file_transfer::staging::{BufferedBoundChunkRead, ChunkStaging, ChunkStagingError},
    },
};
use tokio::sync::Semaphore;

/// Clones share a fail-fast budget for work and returned bytes; independent instances do not.
#[derive(Clone)]
pub struct SessionFileChunkReadWorkspace {
    downloads: SessionFileDownloadWorkspace,
    staging: ChunkStaging,
    controls: Arc<Semaphore>,
}

impl SessionFileChunkReadWorkspace {
    /// Cached trusted configuration only, never permission to read a file or select its owner.
    pub(crate) fn file_scope(&self) -> WorkspaceRef {
        self.downloads.publication.scope()
    }
}

#[derive(Debug, Clone)]
pub struct SessionFileChunkReadRequest {
    pub download: SessionFileDownloadRequest,
    pub chunk_index: u32,
}

/// Move-only verified bytes. Keeping this value keeps its configured response capacity occupied.
///
/// ```compile_fail
/// use cyanrex_engine::services::auth_service::durable_source::SessionFileChunkRead;
/// fn copy(value: SessionFileChunkRead) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use cyanrex_engine::services::auth_service::durable_source::SessionFileChunkRead;
/// fn detach(value: SessionFileChunkRead) { let _ = value.into_vec(); }
/// ```
/// ```compile_fail
/// use cyanrex_engine::services::auth_service::durable_source::SessionFileChunkRead;
/// fn serialize(value: SessionFileChunkRead) { let _ = serde_json::to_vec(&value); }
/// ```
pub struct SessionFileChunkRead {
    file: FileVersionRef,
    index: u32,
    sha256: Sha256Digest,
    proof: BufferedBoundChunkRead,
}
impl SessionFileChunkRead {
    pub fn file(&self) -> &FileVersionRef {
        &self.file
    }
    pub fn chunk_index(&self) -> u32 {
        self.index
    }
    pub fn bytes(&self) -> &[u8] {
        self.proof.bytes()
    }
    /// The selected manifest digest already verified against these bytes, not a new whole digest.
    pub fn chunk_sha256(&self) -> &Sha256Digest {
        &self.sha256
    }
}
impl std::fmt::Debug for SessionFileChunkRead {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SessionFileChunkRead")
            .field("file", &self.file)
            .field("chunk_index", &self.index)
            .field("bytes", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionFileChunkReadError {
    #[error(transparent)]
    Metadata(#[from] SessionFileDownloadError),
    #[error("invalid chunk read input or configuration")]
    InvalidInput,
    #[error("chunk read capacity exhausted")]
    CapacityExhausted,
    #[error("verified chunk bytes unavailable")]
    Unavailable,
}
type ReadResult<T> = std::result::Result<T, SessionFileChunkReadError>;
impl From<ChunkStagingError> for SessionFileChunkReadError {
    fn from(error: ChunkStagingError) -> Self {
        match error {
            ChunkStagingError::InvalidInput => Self::InvalidInput,
            ChunkStagingError::CapacityExhausted => Self::CapacityExhausted,
            _ => Self::Unavailable,
        }
    }
}

impl DurableAuthSource {
    /// Inert host configuration. No storage, permission or connection is installed here.
    pub fn file_chunk_read_workspace(
        &self,
        downloads: SessionFileDownloadWorkspace,
        staging: ChunkStaging,
        capacity: usize,
    ) -> ReadResult<SessionFileChunkReadWorkspace> {
        self.check_file_download_scope(&downloads)?;
        if !(1..=16).contains(&capacity) || staging.scope() != downloads.publication.scope() {
            return Err(SessionFileChunkReadError::InvalidInput);
        }
        Ok(SessionFileChunkReadWorkspace {
            downloads,
            staging,
            controls: Arc::new(Semaphore::new(capacity)),
        })
    }

    pub async fn read_session_file_chunk(
        &self,
        token: &str,
        workspace: &SessionFileChunkReadWorkspace,
        request: &SessionFileChunkReadRequest,
    ) -> ReadResult<SessionFileChunkRead> {
        if !valid_token(token) {
            return Err(SessionFileDownloadError::from(SessionCommandError::InvalidSession).into());
        }
        self.check_file_download_scope(&workspace.downloads)?;
        if request.download.expected_file.workspace() != workspace.staging.scope() {
            return Err(SessionFileDownloadError::from(
                FileRecipientGrantStoreError::ScopeMismatch,
            )
            .into());
        }
        if request.chunk_index as usize >= MAX_FILE_TRANSFER_CHUNKS {
            return Err(SessionFileChunkReadError::InvalidInput);
        }
        let lease = workspace
            .controls
            .clone()
            .try_acquire_owned()
            .map_err(|_| SessionFileChunkReadError::CapacityExhausted)?;
        let recipients = &workspace.downloads.recipients.store;
        let publication = &workspace.downloads.publication;
        let download = &request.download;
        let (mut context, pending, published, manifest, window, root_key) =
            tokio::time::timeout(Duration::from_secs(10), async {
                let mut tx = self.transaction().await?;
                // Fresh snapshots after waits, preserving the configured READ ONLY policy.
                sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                    .execute(&mut *tx)
                    .await?;
                let mut context = self
                    .begin_private_work(&mut tx, token, recipients.namespace(), recipients.scope())
                    .await?;
                let recipient = FileAccountRef::new(context.owner, context.account_id());
                let pending = recipients
                    .get_in_transaction(&mut tx, download.grant_id, recipient)
                    .await?;
                let grant = pending
                    .record()
                    .ok_or(SessionFileDownloadError::PermissionDenied)?
                    .grant();
                if grant.state() != FileGrantState::Active
                    || grant.recipient() != recipient
                    || grant.file() != &download.expected_file
                {
                    return Err(SessionFileDownloadError::PermissionDenied);
                }
                let publisher = grant.publisher();
                let window = grant.validity();
                context
                    .select_target(&mut tx, publication.namespace())
                    .await?;
                let published = publication
                    .get_exact_in_transaction(
                        &mut tx,
                        download.upload_id,
                        publisher.principal(),
                        publisher.account_id(),
                        grant.file(),
                    )
                    .await?;
                let record = published
                    .record()
                    .ok_or(SessionFileDownloadError::PermissionDenied)?;
                let file = FileVersionRef::from_manifest(record.manifest());
                if record.owner() != publisher.principal()
                    || record.account_id() != publisher.account_id()
                    || &file != grant.file()
                    || file != download.expected_file
                {
                    return Err(SessionFileDownloadError::PermissionDenied);
                }
                let manifest = record.manifest().clone();
                let (source_key, root_key) = published
                    .source_root_binding()
                    .ok_or(SessionFileDownloadError::PermissionDenied)?;
                if workspace.staging.bind_upload(source_key) != root_key {
                    return Err(SessionFileDownloadError::PermissionDenied);
                }
                publication
                    .verify_pending_in_transaction(&mut tx, &published)
                    .await?;
                context
                    .select_target(&mut tx, recipients.namespace())
                    .await?;
                recipients
                    .verify_pending_in_transaction(&mut tx, &pending)
                    .await?;
                // Current time eligibility is required before dispatch, not just before disclosure.
                if !context
                    .finish_retained_in_window(
                        self,
                        &mut tx,
                        window.not_before(),
                        window.expires_at(),
                    )
                    .await?
                {
                    return Err(SessionFileDownloadError::PermissionDenied);
                }
                tx.commit().await?;
                Ok::<_, SessionFileDownloadError>((
                    context, pending, published, manifest, window, root_key,
                ))
            })
            .await
            .map_err(|_| SessionFileDownloadError::from(DurableAuthError::StorageUnavailable))??;
        // Check local/empty-file indexes against the server-selected authorized manifest.
        let sha256 = manifest
            .chunk(request.chunk_index)
            .map_err(|_| SessionFileChunkReadError::InvalidInput)?
            .sha256;
        // No database connection or locks are held across this private bounded file work.
        // The control lease moves into the actual job, then its private verified byte result.
        let proof = workspace
            .staging
            .read_bound_chunk(root_key, &manifest, request.chunk_index, lease)
            .await?;
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.transaction().await?;
            sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .execute(&mut *tx)
                .await?;
            // Do not begin a new identity observation or adopt replacement target records.
            context.resume(self, &mut tx).await?;
            recipients
                .refence_pending_in_transaction(&mut tx, &pending)
                .await?;
            context
                .select_target(&mut tx, publication.namespace())
                .await?;
            publication
                .refence_pending_in_transaction(&mut tx, &published)
                .await?;
            publication
                .verify_pending_in_transaction(&mut tx, &published)
                .await?;
            context
                .select_target(&mut tx, recipients.namespace())
                .await?;
            recipients
                .verify_pending_in_transaction(&mut tx, &pending)
                .await?;
            if !context
                .finish_in_window(self, &mut tx, window.not_before(), window.expires_at())
                .await?
            {
                return Err(SessionFileDownloadError::PermissionDenied);
            }
            tx.commit().await?;
            Ok::<_, SessionFileDownloadError>(())
        })
        .await
        .map_err(|_| SessionFileDownloadError::from(DurableAuthError::StorageUnavailable))??;
        // No more IO or SQL after final authorization and acknowledged COMMIT. These are the
        // immutable bytes verified during the read, not a claim about later pathname health.
        Ok(SessionFileChunkRead {
            file: FileVersionRef::from_manifest(&manifest),
            index: request.chunk_index,
            sha256,
            proof,
        })
    }
}
