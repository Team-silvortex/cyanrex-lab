//! Current-Session owner-private metadata only, not a publisher or recipient grant.
use super::{
    private_work::{valid_namespace, PrivateWorkError},
    *,
};
use crate::{
    models::collaboration::{FileTransferManifest, FileUploadId, WorkspaceRef},
    services::{
        file_upload_publication::{
            FileUploadPublicationRecord, FileUploadPublicationStore,
            FileUploadPublicationStoreError, PendingFileUploadPublicationRead,
        },
        file_upload_store::{
            FileUploadLimits, FileUploadReservation, FileUploadStore, FileUploadStoreError,
            PendingFileUpload,
        },
    },
};

/// Trusted configuration only. Ownership/account incarnation are never caller-selected.
#[derive(Clone)]
pub struct SessionFileUploadWorkspace {
    store: FileUploadStore,
}
/// Trusted metadata configuration, not publication permission or an original-byte receipt.
#[derive(Clone)]
pub struct SessionFilePublicationWorkspace {
    store: FileUploadPublicationStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionFileMetadataError {
    #[error(transparent)]
    Session(#[from] SessionCommandError),
    #[error(transparent)]
    Upload(#[from] FileUploadStoreError),
    #[error(transparent)]
    Publication(#[from] FileUploadPublicationStoreError),
    #[error("session file metadata requires separate pinned non-system namespaces")]
    InvalidNamespace,
}
type FileResult<T> = std::result::Result<T, SessionFileMetadataError>;
impl From<PrivateWorkError> for SessionFileMetadataError {
    fn from(error: PrivateWorkError) -> Self {
        match error {
            PrivateWorkError::Session(error) => Self::Session(error),
            PrivateWorkError::InvalidNamespace => Self::InvalidNamespace,
        }
    }
}
impl From<DurableAuthError> for SessionFileMetadataError {
    fn from(error: DurableAuthError) -> Self {
        Self::Session(error.into())
    }
}
impl From<sqlx::Error> for SessionFileMetadataError {
    fn from(_: sqlx::Error) -> Self {
        DurableAuthError::StorageUnavailable.into()
    }
}
enum UploadOperation<'a> {
    Reserve(&'a FileTransferManifest),
    Read,
}

impl DurableAuthSource {
    /// Configuration validation only: no connection, installation, path or byte I/O.
    pub fn file_upload_workspace(
        &self,
        namespace: &str,
        scope: WorkspaceRef,
        installation_id: Uuid,
        limits: FileUploadLimits,
    ) -> FileResult<SessionFileUploadWorkspace> {
        if !valid_namespace(namespace) {
            return Err(SessionFileMetadataError::InvalidNamespace);
        }
        Ok(SessionFileUploadWorkspace {
            store: FileUploadStore::new(
                self.pool.clone(),
                scope,
                installation_id,
                namespace,
                limits,
            )?,
        })
    }
    /// Own Published metadata is not a historical receipt, byte-health check or download grant.
    pub fn file_publication_workspace(
        &self,
        namespace: &str,
        scope: WorkspaceRef,
        installation_id: Uuid,
        max_publications: u32,
    ) -> FileResult<SessionFilePublicationWorkspace> {
        if !valid_namespace(namespace) {
            return Err(SessionFileMetadataError::InvalidNamespace);
        }
        Ok(SessionFilePublicationWorkspace {
            store: FileUploadPublicationStore::new(
                self.pool.clone(),
                scope,
                installation_id,
                namespace,
                max_publications,
            )?,
        })
    }
    /// Reserve claimed metadata for the current owner, without files or publishing authority.
    /// An unavailable result may race COMMIT; it is not rollback proof or permission to retry.
    pub async fn reserve_session_file_upload(
        &self,
        token: &str,
        workspace: &SessionFileUploadWorkspace,
        upload_id: FileUploadId,
        manifest: &FileTransferManifest,
    ) -> FileResult<FileUploadReservation> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        if manifest.workspace() != workspace.store.scope() {
            return Err(FileUploadStoreError::ScopeMismatch.into());
        }
        self.session_file_upload(
            token,
            workspace,
            upload_id,
            UploadOperation::Reserve(manifest),
        )
        .await?
        .ok_or(FileUploadStoreError::InvalidRecord.into())
    }
    /// Selected own metadata only; absence does not lock a future reservation out.
    pub async fn get_session_file_upload(
        &self,
        token: &str,
        workspace: &SessionFileUploadWorkspace,
        upload_id: FileUploadId,
    ) -> FileResult<Option<FileUploadReservation>> {
        self.session_file_upload(token, workspace, upload_id, UploadOperation::Read)
            .await
    }
    /// Inert Published metadata only: does not invoke the publisher or inspect staging bytes.
    pub async fn get_session_file_publication(
        &self,
        token: &str,
        workspace: &SessionFilePublicationWorkspace,
        upload_id: FileUploadId,
    ) -> FileResult<Option<FileUploadPublicationRecord>> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        if workspace.store.scope().authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.file_metadata_transaction().await?;
            let context = self
                .begin_private_work(
                    &mut tx,
                    token,
                    workspace.store.namespace(),
                    workspace.store.scope(),
                )
                .await?;
            let pending: PendingFileUploadPublicationRead = workspace
                .store
                .get_in_transaction(&mut tx, upload_id, context.owner, context.account_id())
                .await?;
            workspace
                .store
                .verify_pending_in_transaction(&mut tx, &pending)
                .await?;
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
        .map_err(|_| SessionFileMetadataError::from(DurableAuthError::StorageUnavailable))?
    }
    async fn session_file_upload(
        &self,
        token: &str,
        workspace: &SessionFileUploadWorkspace,
        upload_id: FileUploadId,
        operation: UploadOperation<'_>,
    ) -> FileResult<Option<FileUploadReservation>> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        if workspace.store.scope().authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.file_metadata_transaction().await?;
            let context = self
                .begin_private_work(
                    &mut tx,
                    token,
                    workspace.store.namespace(),
                    workspace.store.scope(),
                )
                .await?;
            let reserve = matches!(operation, UploadOperation::Reserve(_));
            let pending: PendingFileUpload = match operation {
                UploadOperation::Reserve(manifest) => {
                    workspace
                        .store
                        .reserve_in_transaction(
                            &mut tx,
                            upload_id,
                            context.owner,
                            context.account_id(),
                            manifest,
                        )
                        .await?
                }
                UploadOperation::Read => {
                    workspace
                        .store
                        .get_in_transaction(&mut tx, upload_id, context.owner, context.account_id())
                        .await?
                }
            };
            if reserve {
                // Triggers may change settings: flush deferred writes before exact ledger checks.
                sqlx::query("SET LOCAL synchronous_commit=on")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
                    .execute(&mut *tx)
                    .await?;
            }
            workspace
                .store
                .verify_pending_in_transaction(&mut tx, &pending)
                .await?;
            context.finish(self, &mut tx).await?;
            if reserve {
                sqlx::query("SET LOCAL synchronous_commit=on")
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
            // No metadata is returned before the final Session check and commit acknowledgement.
            Ok(pending.into_record())
        })
        .await
        .map_err(|_| SessionFileMetadataError::from(DurableAuthError::StorageUnavailable))?
    }
    async fn file_metadata_transaction(&self) -> FileResult<Transaction<'_, Postgres>> {
        let mut tx = self.transaction().await?;
        // Must precede the guard's first snapshot; independent pools may default to RR.
        // Do not override a configured READ ONLY source; the locking guard must fail closed.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
}
