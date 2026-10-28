//! Explicit Task-bound text publication only. No route, Task write, pool or browser connection.
use super::PlatformStorageConnector;
use crate::{
    models::collaboration::{ArtifactId, ArtifactRevision, ArtifactRevisionId, WorkspaceRef},
    services::{
        artifact_store::ArtifactContentWriter,
        auth_service::durable_source::{
            task_text_artifact_transaction::{
                PreparedTaskTextArtifactPublish, TaskTextArtifactPublishWorkspace,
            },
            valid_token, DurableAuthError, SessionCommandError, SessionTaskError,
        },
        task_store::TaskContentSnapshot,
    },
};
use sqlx_core::connection::Connection;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

pub struct PlatformTaskTextArtifactConnector {
    storage: Arc<PlatformStorageConnector>,
    workers: Arc<Semaphore>,
}

impl PlatformTaskTextArtifactConnector {
    /// Pure, explicitly selected capability over one original storage binding.
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

    /// The supplied baseline is an exact stale-input fence, never an owner selection or grant.
    /// Publication does not attach the returned revision to the Task or advance that Task.
    pub async fn create_session_task_text_artifact(
        &self,
        token: &str,
        expected: TaskContentSnapshot,
        artifact: ArtifactId,
        revision: ArtifactRevisionId,
        text: String,
    ) -> Result<ArtifactRevision, SessionTaskError> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        let draft = TaskTextArtifactPublishWorkspace::draft(self.scope(), &expected, text)?;
        let mut connection = self
            .storage
            .connect()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            let workspace = TaskTextArtifactPublishWorkspace::new(
                self.storage.config().namespaces().task(),
                self.storage.config().namespaces().artifacts(),
                self.scope(),
                ArtifactContentWriter::from_reader(
                    self.storage.content_reader(Arc::clone(&self.workers))?,
                ),
            )?;
            let mut tx = connection
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await
                .map_err(DurableAuthError::from)?;
            self.verify(&mut tx).await?;
            let prepared = PreparedTaskTextArtifactPublish::prepare(
                self.scope().authority_id,
                &mut tx,
                token,
                &workspace,
                expected,
                artifact,
                revision,
                draft,
            )
            .await?;
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
        // Confirmed COMMIT survives a failed close; uncertainty never grants file cleanup/retry.
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
