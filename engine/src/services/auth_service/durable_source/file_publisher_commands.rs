//! Current-Session own publisher-grant metadata, never permission issuance or byte access.
use super::{
    private_work::{valid_namespace, PrivateWorkError},
    *,
};
use crate::{
    models::collaboration::{FileAccountRef, FileGrantId, WorkspaceRef},
    services::file_publisher_grant_store::{
        FilePublisherGrantStore, FilePublisherGrantStoreError, PendingFilePublisherGrantRead,
        StoredFilePublisherGrant,
    },
};

/// Frozen trusted metadata configuration. No installer, role conversion or publication gate.
#[derive(Clone)]
pub struct SessionFilePublisherGrantWorkspace {
    pub(super) store: FilePublisherGrantStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionFilePublisherGrantError {
    #[error(transparent)]
    Session(#[from] SessionCommandError),
    #[error(transparent)]
    Grant(#[from] FilePublisherGrantStoreError),
    #[error("publisher grant metadata requires separate pinned non-system namespaces")]
    InvalidNamespace,
    #[error("current account is not the configured file-grant manager")]
    PermissionDenied,
}
pub(super) type GrantResult<T> = std::result::Result<T, SessionFilePublisherGrantError>;
impl From<PrivateWorkError> for SessionFilePublisherGrantError {
    fn from(error: PrivateWorkError) -> Self {
        match error {
            PrivateWorkError::Session(error) => Self::Session(error),
            PrivateWorkError::InvalidNamespace => Self::InvalidNamespace,
        }
    }
}
impl From<DurableAuthError> for SessionFilePublisherGrantError {
    fn from(error: DurableAuthError) -> Self {
        Self::Session(error.into())
    }
}
impl From<sqlx::Error> for SessionFilePublisherGrantError {
    fn from(_: sqlx::Error) -> Self {
        DurableAuthError::StorageUnavailable.into()
    }
}

impl DurableAuthSource {
    /// Freezes scope, installation and budget without connecting or installing any schema.
    pub fn file_publisher_grant_workspace(
        &self,
        namespace: &str,
        scope: WorkspaceRef,
        installation_id: Uuid,
        max_grants: u32,
    ) -> GrantResult<SessionFilePublisherGrantWorkspace> {
        if !valid_namespace(namespace) {
            return Err(SessionFilePublisherGrantError::InvalidNamespace);
        }
        Ok(SessionFilePublisherGrantWorkspace {
            store: FilePublisherGrantStore::new(
                self.pool.clone(),
                scope,
                installation_id,
                namespace,
                max_grants,
            )?,
        })
    }

    /// Selected own metadata only. Active state does not authorize later publication or sharing.
    pub async fn get_session_file_publisher_grant(
        &self,
        token: &str,
        workspace: &SessionFilePublisherGrantWorkspace,
        grant_id: FileGrantId,
    ) -> GrantResult<Option<StoredFilePublisherGrant>> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        if workspace.store.scope().authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.transaction().await?;
            // Fresh reads after target waits; preserve the configured READ ONLY policy.
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
            let subject = FileAccountRef::new(context.owner, context.account_id());
            let pending: PendingFilePublisherGrantRead = workspace
                .store
                .get_in_transaction(&mut tx, grant_id, subject)
                .await?;
            workspace
                .store
                .verify_pending_in_transaction(&mut tx, &pending)
                .await?;
            // The selected grant/history write fence remains held through final authorization
            // and commit. Active metadata returned here is not permission for later byte work.
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
        .map_err(|_| SessionFilePublisherGrantError::from(DurableAuthError::StorageUnavailable))?
    }
}
