//! Current-Session own recipient metadata; never download permission or grant management.
use super::{
    private_work::{valid_namespace, PrivateWorkError},
    *,
};
use crate::{
    models::collaboration::{FileAccountRef, FileGrantId, WorkspaceRef},
    services::file_recipient_grant_store::{
        FileRecipientGrantStore, FileRecipientGrantStoreError, PendingFileRecipientGrantRead,
        StoredFileRecipientGrant,
    },
};

/// Inert trusted configuration, not an installer or current authorization.
#[derive(Clone)]
pub struct SessionFileRecipientGrantWorkspace {
    pub(super) store: FileRecipientGrantStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionFileRecipientGrantError {
    #[error(transparent)]
    Session(#[from] SessionCommandError),
    #[error(transparent)]
    Grant(#[from] FileRecipientGrantStoreError),
    #[error("recipient metadata requires separate pinned non-system namespaces")]
    InvalidNamespace,
}
type GrantResult<T> = std::result::Result<T, SessionFileRecipientGrantError>;
impl From<PrivateWorkError> for SessionFileRecipientGrantError {
    fn from(error: PrivateWorkError) -> Self {
        match error {
            PrivateWorkError::Session(error) => Self::Session(error),
            PrivateWorkError::InvalidNamespace => Self::InvalidNamespace,
        }
    }
}
impl From<DurableAuthError> for SessionFileRecipientGrantError {
    fn from(error: DurableAuthError) -> Self {
        Self::Session(error.into())
    }
}
impl From<sqlx::Error> for SessionFileRecipientGrantError {
    fn from(_: sqlx::Error) -> Self {
        DurableAuthError::StorageUnavailable.into()
    }
}

impl DurableAuthSource {
    /// Freezes a trusted scope and installation without connecting or installing anything.
    pub fn file_recipient_grant_workspace(
        &self,
        namespace: &str,
        scope: WorkspaceRef,
        installation_id: Uuid,
        max_grants: u32,
    ) -> GrantResult<SessionFileRecipientGrantWorkspace> {
        if !valid_namespace(namespace) {
            return Err(SessionFileRecipientGrantError::InvalidNamespace);
        }
        Ok(SessionFileRecipientGrantWorkspace {
            store: FileRecipientGrantStore::new(
                self.pool.clone(),
                scope,
                installation_id,
                namespace,
                max_grants,
            )?,
        })
    }
    /// Own selected metadata only. Neither Active state nor validity grants later byte access.
    pub async fn get_session_file_recipient_grant(
        &self,
        token: &str,
        workspace: &SessionFileRecipientGrantWorkspace,
        grant_id: FileGrantId,
    ) -> GrantResult<Option<StoredFileRecipientGrant>> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        if workspace.store.scope().authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.transaction().await?;
            // Fresh snapshots after waits, without overriding configured READ ONLY policy.
            sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .execute(&mut *tx)
                .await?;
            let context = self
                .begin_private_work(
                    &mut tx,
                    token,
                    workspace.store.namespace(),
                    workspace.store.scope(),
                )
                .await?;
            let recipient = FileAccountRef::new(context.owner, context.account_id());
            let pending: PendingFileRecipientGrantRead = workspace
                .store
                .get_in_transaction(&mut tx, grant_id, recipient)
                .await?;
            workspace
                .store
                .verify_pending_in_transaction(&mut tx, &pending)
                .await?;
            // Original selected history and its write fence survive final current authority and
            // acknowledgement. The returned metadata is not a cached download permission.
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
        .map_err(|_| SessionFileRecipientGrantError::from(DurableAuthError::StorageUnavailable))?
    }
}
