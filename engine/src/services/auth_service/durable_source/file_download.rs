//! Current-recipient exact Published manifest observation, never a reusable byte-access token.
use super::{
    private_work::{valid_namespace, PrivateWorkError},
    *,
};
use crate::{
    models::collaboration::{
        FileAccountRef, FileGrantId, FileGrantState, FileTransferManifest, FileUploadId,
        FileVersionRef,
    },
    services::{
        file_recipient_grant_store::FileRecipientGrantStoreError,
        file_upload_publication::{FileUploadPublicationStore, FileUploadPublicationStoreError},
    },
};

/// Inert trusted host configuration. No installation or permission is created by construction.
#[derive(Clone)]
pub struct SessionFileDownloadWorkspace {
    pub(super) publication: FileUploadPublicationStore,
    pub(super) recipients: SessionFileRecipientGrantWorkspace,
}

impl SessionFileDownloadWorkspace {
    /// Frozen trusted scope only; this getter neither reads storage nor grants access.
    #[cfg(unix)]
    pub(crate) fn file_scope(&self) -> crate::models::collaboration::WorkspaceRef {
        self.publication.scope()
    }
}

/// Exact selectors only. Recipient and publisher attribution are never supplied by this request.
#[derive(Debug, Clone)]
pub struct SessionFileDownloadRequest {
    pub grant_id: FileGrantId,
    pub upload_id: FileUploadId,
    pub expected_file: FileVersionRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionFileDownloadError {
    #[error(transparent)]
    Session(#[from] SessionCommandError),
    #[error(transparent)]
    Recipient(#[from] FileRecipientGrantStoreError),
    #[error(transparent)]
    Publication(#[from] FileUploadPublicationStoreError),
    #[error("download metadata requires two distinct pinned non-system namespaces")]
    InvalidNamespace,
    #[error("current recipient cannot observe this exact Published manifest")]
    PermissionDenied,
}
type DownloadResult<T> = std::result::Result<T, SessionFileDownloadError>;
impl From<PrivateWorkError> for SessionFileDownloadError {
    fn from(error: PrivateWorkError) -> Self {
        match error {
            PrivateWorkError::Session(error) => Self::Session(error),
            PrivateWorkError::InvalidNamespace => Self::InvalidNamespace,
        }
    }
}
impl From<DurableAuthError> for SessionFileDownloadError {
    fn from(error: DurableAuthError) -> Self {
        Self::Session(error.into())
    }
}
impl From<sqlx::Error> for SessionFileDownloadError {
    fn from(_: sqlx::Error) -> Self {
        DurableAuthError::StorageUnavailable.into()
    }
}

impl DurableAuthSource {
    pub fn file_download_workspace(
        &self,
        publication: FileUploadPublicationStore,
        recipients: SessionFileRecipientGrantWorkspace,
    ) -> DownloadResult<SessionFileDownloadWorkspace> {
        let workspace = SessionFileDownloadWorkspace {
            publication,
            recipients,
        };
        self.check_file_download_scope(&workspace)?;
        let publication = workspace.publication.namespace();
        let recipients = workspace.recipients.store.namespace();
        if !valid_namespace(publication)
            || !valid_namespace(recipients)
            || publication == recipients
        {
            return Err(SessionFileDownloadError::InvalidNamespace);
        }
        Ok(workspace)
    }

    /// One current eligibility observation. The returned manifest cannot authorize later bytes.
    pub async fn get_session_file_download_manifest(
        &self,
        token: &str,
        workspace: &SessionFileDownloadWorkspace,
        request: &SessionFileDownloadRequest,
    ) -> DownloadResult<FileTransferManifest> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        self.check_file_download_scope(workspace)?;
        if request.expected_file.workspace() != workspace.publication.scope() {
            return Err(FileRecipientGrantStoreError::ScopeMismatch.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.transaction().await?;
            // Fresh snapshots after waits, without overriding configured READ ONLY policy.
            sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .execute(&mut *tx)
                .await?;
            let recipients = &workspace.recipients.store;
            let publication = &workspace.publication;
            let mut context = self
                .begin_private_work(&mut tx, token, recipients.namespace(), recipients.scope())
                .await?;
            let recipient = FileAccountRef::new(context.owner, context.account_id());
            let pending = recipients
                .get_in_transaction(&mut tx, request.grant_id, recipient)
                .await?;
            let selected = pending
                .record()
                .ok_or(SessionFileDownloadError::PermissionDenied)?;
            let grant = selected.grant();
            if grant.state() != FileGrantState::Active
                || grant.recipient() != recipient
                || grant.file() != &request.expected_file
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
                    request.upload_id,
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
                || file != request.expected_file
            {
                return Err(SessionFileDownloadError::PermissionDenied);
            }
            // Private local metadata only; nothing escapes before original verification and ACK.
            let manifest = record.manifest().clone();
            publication
                .verify_pending_in_transaction(&mut tx, &published)
                .await?;
            context
                .select_target(&mut tx, recipients.namespace())
                .await?;
            recipients
                .verify_pending_in_transaction(&mut tx, &pending)
                .await?;
            // No early grant clock is adopted as permission. After ALL original record/source
            // waits, one final sample covers the recipient window and exact Session expiry.
            // No later SQL before requesting COMMIT; a manifest is never authority for later IO.
            if !context
                .finish_in_window(self, &mut tx, window.not_before(), window.expires_at())
                .await?
            {
                return Err(SessionFileDownloadError::PermissionDenied);
            }
            tx.commit().await?;
            Ok(manifest)
        })
        .await
        .map_err(|_| SessionFileDownloadError::from(DurableAuthError::StorageUnavailable))?
    }

    pub(super) fn check_file_download_scope(
        &self,
        workspace: &SessionFileDownloadWorkspace,
    ) -> DownloadResult<()> {
        let scope = workspace.publication.scope();
        if scope.authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        if scope != workspace.recipients.store.scope() {
            return Err(FileRecipientGrantStoreError::ScopeMismatch.into());
        }
        Ok(())
    }
}
