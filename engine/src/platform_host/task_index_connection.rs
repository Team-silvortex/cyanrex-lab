//! Owner-private Task metadata over the original retained storage binding.
use super::PlatformStorageConnector;
use crate::{
    models::collaboration::{TaskRef, WorkspaceRef},
    services::{
        auth_service::durable_source::{
            task_index_transaction::PreparedTaskIndexRead, DurableAuthError,
            SessionTaskContentIndexWorkspace, SessionTaskError,
        },
        task_store::TaskContentIndexPage,
    },
};
use sqlx_core::connection::Connection;
use std::{sync::Arc, time::Duration};

/// Complete reads only: no pool, baseline export, caller-selected scope or reusable grant.
pub struct PlatformTaskIndexConnector {
    storage: Arc<PlatformStorageConnector>,
}

impl PlatformTaskIndexConnector {
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

    pub async fn list_session_content_tasks(
        &self,
        token: &str,
        after: Option<TaskRef>,
        limit: Option<usize>,
    ) -> Result<TaskContentIndexPage, SessionTaskError> {
        let workspace = SessionTaskContentIndexWorkspace::new(
            self.storage.config().namespaces().task(),
            self.scope(),
        )?;
        PreparedTaskIndexRead::validate(
            self.scope().authority_id,
            token,
            &workspace,
            after,
            limit,
        )?;
        let mut connection = self
            .storage
            .connect()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            // Fresh final checks need READ COMMITTED. Never override explicit READ ONLY policy.
            let mut tx = connection
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await
                .map_err(DurableAuthError::from)?;
            self.verify(&mut tx).await?;
            let prepared = PreparedTaskIndexRead::prepare(
                self.scope().authority_id,
                &mut tx,
                token,
                &workspace,
                after,
                limit,
            )
            .await?;
            // prepare restored the original Source. All retained guard waits must finish before
            // finish's fresh exact Session/account/membership check, not after authorization.
            self.verify(&mut tx).await?;
            let pending = prepared.finish(&mut tx).await?;
            tx.commit().await.map_err(DurableAuthError::from)?;
            Ok(pending)
        })
        .await
        .unwrap_or(Err(DurableAuthError::StorageUnavailable.into()));
        // Independent close budget; confirmed COMMIT is not undone by a close failure. A timeout
        // or cancelled future drops connection state, never proof of a remote rollback outcome.
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
