//! Explicit existing-Session operations over one retained storage binding; no pool or issuer.
use super::PlatformStorageConnector;
use crate::services::auth_service::durable_source::{
    session_transaction::{PreparedSessionLogout, PreparedSessionRead},
    valid_token, DurableAuthError, DurableSession,
};
use sqlx_core::connection::Connection;
use std::{sync::Arc, time::Duration};

/// Complete commands only. No Clone/Debug, pool, credential export, baseline export or half-command API.
pub struct PlatformSessionConnector {
    storage: Arc<PlatformStorageConnector>,
}

impl PlatformSessionConnector {
    /// Pure composition. Authority and Origin come only from the original storage configuration.
    pub fn new(storage: PlatformStorageConnector) -> Self {
        Self::from_shared(Arc::new(storage))
    }

    pub(super) fn from_shared(storage: Arc<PlatformStorageConnector>) -> Self {
        Self { storage }
    }

    pub fn origin(&self) -> &str {
        self.storage.config().origin()
    }

    /// Fresh Session snapshot, not permission for any later Task/Artifact operation.
    pub async fn validate_session(
        &self,
        token: &str,
    ) -> Result<Option<DurableSession>, DurableAuthError> {
        if !valid_token(token) {
            return Ok(None);
        }
        let mut connection = self
            .storage
            .connect()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            // READ COMMITTED keeps rechecks fresh; never override explicit READ ONLY policy.
            let mut tx = connection
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await?;
            self.verify(&mut tx).await?;
            let prepared = PreparedSessionRead::prepare(
                self.storage.config().scope().authority_id,
                &mut tx,
                token,
            )
            .await?;
            self.verify(&mut tx).await?;
            let session = prepared.finish(&mut tx).await?;
            tx.commit().await?;
            Ok(session)
        })
        .await
        .unwrap_or(Err(DurableAuthError::StorageUnavailable));
        close(connection).await;
        result
    }

    /// Delete only this exact token. Canonical unknown/expired tokens can confirm absence.
    pub async fn logout(&self, token: &str) -> Result<(), DurableAuthError> {
        if !valid_token(token) {
            return Err(DurableAuthError::InvalidInput);
        }
        let mut connection = self
            .storage
            .connect()
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let result = tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = connection
                .begin_with("BEGIN ISOLATION LEVEL READ COMMITTED")
                .await?;
            self.verify(&mut tx).await?;
            let prepared = PreparedSessionLogout::prepare(
                self.storage.config().scope().authority_id,
                &mut tx,
                token,
            )
            .await?;
            // Fire deferred trigger work before retained-target and final absence checks.
            crate::sqlx_compat::query("SET CONSTRAINTS ALL IMMEDIATE")
                .execute(&mut *tx)
                .await?;
            self.verify(&mut tx).await?;
            prepared.finish(&mut tx).await?;
            tx.commit().await?;
            Ok(())
        })
        .await
        .unwrap_or(Err(DurableAuthError::StorageUnavailable));
        close(connection).await;
        result
    }

    async fn verify(
        &self,
        tx: &mut sqlx_core::transaction::Transaction<'_, crate::sqlx_compat::Postgres>,
    ) -> Result<(), DurableAuthError> {
        self.storage
            .verify_transaction(tx)
            .await
            .map_err(|_| DurableAuthError::StorageUnavailable)
    }
}

async fn close(connection: sqlx_postgres::PgConnection) {
    // Independent close budget; a confirmed commit remains confirmed even if close fails.
    // Cancellation/timeouts drop connection state, not proof of the remote rollback outcome.
    let _ = tokio::time::timeout(Duration::from_secs(10), connection.close()).await;
}
