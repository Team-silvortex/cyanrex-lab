//! Explicit whole-manifest Draft replacement over one original retained storage binding.
use super::PlatformStorageConnector;
use crate::{
    models::collaboration::{RevisionNumber, TaskContentManifest, TaskId, WorkspaceRef},
    services::{
        auth_service::durable_source::{
            task_replace_transaction::{PreparedTaskContentReplace, TaskContentReplaceWorkspace},
            valid_token, DurableAuthError, SessionCommandError, SessionTaskError,
        },
        task_store::TaskContentSnapshot,
    },
};
use sqlx_core::connection::Connection;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

/// One write capability only; no publication, caller-selected owner or fallback pool.
pub struct PlatformTaskReplaceConnector {
    storage: Arc<PlatformStorageConnector>,
    workers: Arc<Semaphore>,
}

impl PlatformTaskReplaceConnector {
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

    pub async fn replace_session_content_task(
        &self,
        token: &str,
        id: TaskId,
        revision: RevisionNumber,
        manifest: TaskContentManifest,
    ) -> Result<TaskContentSnapshot, SessionTaskError> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        TaskContentReplaceWorkspace::validate_manifest(self.scope(), &manifest)?;
        let mut connection = self
            .storage
            .connect()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            // The root reader inherits only the original descriptor and bounded actual workers.
            let workspace = TaskContentReplaceWorkspace::new(
                self.storage.config().namespaces().task(),
                self.storage.config().namespaces().artifacts(),
                self.scope(),
                self.storage.content_reader(Arc::clone(&self.workers))?,
            )?;
            // Fresh post-wait authority requires READ COMMITTED; never override READ ONLY.
            let mut tx = connection
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await
                .map_err(DurableAuthError::from)?;
            self.verify(&mut tx).await?;
            let prepared = PreparedTaskContentReplace::prepare(
                self.scope().authority_id,
                &mut tx,
                token,
                &workspace,
                id,
                revision,
                manifest,
            )
            .await?;
            // prepare completed deferred work and post-write old/new verification. All original
            // root/storage waits precede the final fresh exact Session check and immediate commit.
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
        // Confirmed COMMIT survives a failed close. Cancellation/timeout never grants retry,
        // adoption, filesystem cleanup or certainty about remote rollback/blocking-job completion.
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
