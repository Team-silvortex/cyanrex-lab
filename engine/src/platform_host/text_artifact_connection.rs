//! One explicit new-text capability over the original storage. No listener, pool or Task writes.
use super::PlatformStorageConnector;
use crate::{
    models::collaboration::{ArtifactId, ArtifactRevision, ArtifactRevisionId, WorkspaceRef},
    services::{
        artifact_store::ArtifactContentWriter,
        auth_service::durable_source::{
            text_artifact_transaction::{
                PreparedTextArtifactPublish, TextArtifactPublishWorkspace,
            },
            valid_token, DurableAuthError, SessionArtifactError, SessionCommandError,
        },
    },
};
use sqlx_core::connection::Connection;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;

pub struct PlatformTextArtifactConnector {
    storage: Arc<PlatformStorageConnector>,
    workers: Arc<Semaphore>,
}

impl PlatformTextArtifactConnector {
    /// Pure composition; original observations are never opened, exported or updated here.
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

    pub async fn create_session_text_artifact(
        &self,
        token: &str,
        artifact: ArtifactId,
        revision: ArtifactRevisionId,
        title: String,
        text: String,
    ) -> Result<ArtifactRevision, SessionArtifactError> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        let draft = TextArtifactPublishWorkspace::draft(title, text)?;
        let mut connection = self
            .storage
            .connect()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            let workspace = TextArtifactPublishWorkspace::new(
                self.storage.config().namespaces().artifacts(),
                self.scope(),
                ArtifactContentWriter::from_reader(
                    self.storage.content_reader(Arc::clone(&self.workers))?,
                ),
            )?;
            // Fresh post-wait authority without overriding a role's explicit READ ONLY policy.
            let mut tx = connection
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await
                .map_err(DurableAuthError::from)?;
            self.verify(&mut tx).await?;
            let prepared = PreparedTextArtifactPublish::prepare(
                self.scope().authority_id,
                &mut tx,
                token,
                &workspace,
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
        // Only acknowledged COMMIT yields success, independent of close. No failure permits
        // rollback claims, file removal, adoption or retry; dispatched file jobs keep their slots.
        let _ = tokio::time::timeout(Duration::from_secs(10), connection.close()).await;
        result
    }

    async fn verify(
        &self,
        tx: &mut sqlx_core::transaction::Transaction<'_, crate::sqlx_compat::Postgres>,
    ) -> Result<(), SessionArtifactError> {
        self.storage
            .verify_transaction(tx)
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable.into())
    }
}
