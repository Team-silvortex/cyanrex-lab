//! Explicit empty Task creation over one original retained storage binding.
use super::PlatformStorageConnector;
use crate::{
    models::collaboration::{TaskContentManifest, TaskId, WorkspaceRef},
    services::{
        auth_service::durable_source::{
            task_create_transaction::{PreparedTaskCreate, TaskCreateWorkspace},
            valid_token, DurableAuthError, SessionCommandError, SessionTaskError,
        },
        task_store::{TaskContentSnapshot, TaskStoreError},
    },
};
use sqlx_core::connection::Connection;
use std::{sync::Arc, time::Duration};

/// One command only: no pool, caller owner/paths, catalogue, payload publication or replay.
pub struct PlatformTaskCreateConnector {
    storage: Arc<PlatformStorageConnector>,
}

impl PlatformTaskCreateConnector {
    pub fn new(storage: PlatformStorageConnector) -> Self {
        Self::from_shared(Arc::new(storage))
    }

    pub(super) fn from_shared(storage: Arc<PlatformStorageConnector>) -> Self {
        Self { storage }
    }

    pub fn origin(&self) -> &str {
        self.storage.config().origin()
    }

    pub fn scope(&self) -> WorkspaceRef {
        self.storage.config().scope()
    }

    pub async fn create_session_empty_content_task(
        &self,
        token: &str,
        id: TaskId,
        title: &str,
    ) -> Result<TaskContentSnapshot, SessionTaskError> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        let manifest = TaskContentManifest::new(title.to_owned(), vec![])
            .map_err(|_| TaskStoreError::InvalidInput)?;
        let workspace = TaskCreateWorkspace::new(
            self.storage.config().namespaces().task(),
            self.storage.config().namespaces().artifacts(),
            self.scope(),
        )?;
        let mut connection = self
            .storage
            .connect()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            // Fresh final guards require READ COMMITTED; never override an explicit READ ONLY role.
            let mut tx = connection
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await
                .map_err(DurableAuthError::from)?;
            self.verify(&mut tx).await?;
            let prepared = PreparedTaskCreate::prepare(
                self.scope().authority_id,
                &mut tx,
                token,
                &workspace,
                id,
                manifest,
            )
            .await?;
            // prepare flushed deferred work, re-read the expected pair and restored Source.
            // All original storage/root waits precede finish's final fresh exact Session read.
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
        // A confirmed commit stays confirmed if close fails. Cancellation or timeout is neither
        // rollback proof nor permission to retry, adopt an existing Task or clean up resources.
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
