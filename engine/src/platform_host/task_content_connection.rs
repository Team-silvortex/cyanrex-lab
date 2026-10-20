//! Read-only owner content from the original retained platform storage.
use super::PlatformStorageConnector;
use crate::{
    models::collaboration::{TaskId, WorkspaceRef},
    services::auth_service::durable_source::{
        task_content_transaction::{PreparedTaskContentRead, TaskContentReadWorkspace},
        valid_token, DurableAuthError, SessionCommandError, SessionTaskContent, SessionTaskError,
    },
};
use sqlx_core::connection::Connection;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

/// Complete read capability only; no pool, baseline export, caller paths or write methods.
pub struct PlatformTaskContentConnector {
    storage: Arc<PlatformStorageConnector>,
    workers: Arc<Semaphore>,
}

impl PlatformTaskContentConnector {
    pub fn new(storage: PlatformStorageConnector) -> Self {
        Self::from_shared(Arc::new(storage))
    }

    pub(super) fn from_shared(storage: Arc<PlatformStorageConnector>) -> Self {
        Self {
            storage,
            workers: Arc::new(Semaphore::new(4)),
        }
    }

    pub fn origin(&self) -> &str {
        self.storage.config().origin()
    }

    pub fn scope(&self) -> WorkspaceRef {
        self.storage.config().scope()
    }

    pub async fn get_session_content_task(
        &self,
        token: &str,
        id: TaskId,
    ) -> Result<Option<SessionTaskContent>, SessionTaskError> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        let mut connection = self
            .storage
            .connect()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            // Inherit only the original root descriptor, never open a new acceptable baseline.
            let workspace = TaskContentReadWorkspace::new(
                self.storage.config().namespaces().task(),
                self.storage.config().namespaces().artifacts(),
                self.scope(),
                self.storage.content_reader(Arc::clone(&self.workers))?,
            )?;
            // Keep rechecks fresh and never override explicit READ ONLY role policy.
            let mut tx = connection
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await
                .map_err(DurableAuthError::from)?;
            self.verify(&mut tx).await?;
            let prepared = PreparedTaskContentRead::prepare(
                self.scope().authority_id,
                &mut tx,
                token,
                &workspace,
                id,
            )
            .await?;
            // Bytes are still private. Complete root/storage guard waits before final fresh
            // Session/account/membership checks, even for an absent or empty-payload Task.
            workspace.verify_root().await?;
            self.storage
                .verify_artifact_root()
                .map_err(|_| DurableAuthError::StorageUnavailable)?;
            self.verify(&mut tx).await?;
            let pending = prepared.finish(&mut tx).await?;
            tx.commit().await.map_err(DurableAuthError::from)?;
            Ok(pending)
        })
        .await
        .unwrap_or(Err(DurableAuthError::StorageUnavailable.into()));
        // Confirmed COMMIT survives close failure. Cancellation drops local connection state,
        // not proof of remote rollback or termination of already dispatched blocking file work.
        let _ = tokio::time::timeout(Duration::from_secs(10), connection.close()).await;
        result
    }

    async fn verify(
        &self,
        tx: &mut sqlx_core::transaction::Transaction<'_, crate::sqlx_compat::Postgres>,
    ) -> Result<(), SessionTaskError> {
        self.storage
            .verify_transaction(tx)
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable.into())
    }
}
