//! Caller-selected original storage observations, composed without a first-read baseline.
use super::{
    verify_platform_artifact_root, verify_platform_storage_markers,
    PlatformArtifactRootObservation, PlatformDatabaseConnector, PlatformHostPreflightConfig,
    PlatformStorageMarkerObservation,
};
use sqlx_core::connection::Connection;
use sqlx_postgres::PgConnection;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlatformStorageConnectionError {
    #[error("invalid platform storage connection configuration")]
    InvalidConfig,
    #[error("platform storage connection is unavailable or unverified")]
    Unavailable,
}
type Result<T> = std::result::Result<T, PlatformStorageConnectionError>;

/// Trusted in-memory bindings only; no Debug, Clone, serialization or baseline update/export.
pub struct PlatformStorageConnector {
    config: PlatformHostPreflightConfig,
    database: PlatformDatabaseConnector,
    original_markers: PlatformStorageMarkerObservation,
    root_path: PathBuf,
    original_root: PlatformArtifactRootObservation,
}

impl PlatformStorageConnector {
    pub(super) fn content_reader(
        &self,
        workers: std::sync::Arc<tokio::sync::Semaphore>,
    ) -> std::result::Result<
        crate::services::artifact_store::ArtifactContentReader,
        crate::services::artifact_store::ArtifactStoreError,
    > {
        self.original_root.content_reader(workers)
    }

    pub(super) fn verify_artifact_root(&self) -> Result<()> {
        verify_platform_artifact_root(&self.config, &self.root_path, &self.original_root)
            .map_err(|_| PlatformStorageConnectionError::Unavailable)
    }

    /// Crate-only public metadata for validation before splitting host capabilities.
    pub(crate) fn origin(&self) -> &str {
        self.config.origin()
    }

    pub(super) fn config(&self) -> &PlatformHostPreflightConfig {
        &self.config
    }

    pub(super) async fn verify_transaction(
        &self,
        tx: &mut sqlx_core::transaction::Transaction<'_, crate::sqlx_compat::Postgres>,
    ) -> Result<()> {
        super::storage_markers::verify_in_transaction(&self.config, tx, &self.original_markers)
            .await
            .map_err(|_| PlatformStorageConnectionError::Unavailable)
    }

    /// Pure composition of genuine caller-selected observations, not historical discovery.
    /// Incompatible complete configurations, catalog identities or raw path spellings fail.
    pub fn new(
        config: PlatformHostPreflightConfig,
        database: PlatformDatabaseConnector,
        original_markers: PlatformStorageMarkerObservation,
        root_path: &Path,
        original_root: PlatformArtifactRootObservation,
    ) -> Result<Self> {
        if !database.binds_catalog(&config, original_markers.catalog())
            || !original_root.binds_path(&config, root_path)
        {
            return Err(PlatformStorageConnectionError::InvalidConfig);
        }
        Ok(Self {
            config,
            database,
            original_markers,
            root_path: root_path.to_owned(),
            original_root,
        })
    }

    /// Recheck the root, then one fresh catalog/marker-verified connection, then the root again.
    /// Synchronous root checks have no hard deadline and are not a cross-resource atomic lock.
    /// Database work has one ten-second budget; root-postcheck failure has a separate close budget.
    /// The caller prepares its process and retains current transaction-owned business guards.
    pub async fn connect(&self) -> Result<PgConnection> {
        verify_platform_artifact_root(&self.config, &self.root_path, &self.original_root)
            .map_err(|_| PlatformStorageConnectionError::Unavailable)?;
        let database = self.connect_database().await;
        // Even database failure gets a root postcheck. A successful unpublished connection is
        // held during this unbounded synchronous metadata work, never handed out prematurely.
        let root =
            verify_platform_artifact_root(&self.config, &self.root_path, &self.original_root);
        match (database, root) {
            (Ok(connection), Ok(())) => Ok(connection),
            (Ok(connection), Err(_)) => {
                close(connection).await;
                Err(PlatformStorageConnectionError::Unavailable)
            }
            (Err(_), _) => Err(PlatformStorageConnectionError::Unavailable),
        }
    }

    async fn connect_database(&self) -> Result<PgConnection> {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut connection = self
                .database
                .connect()
                .await
                .map_err(|_| PlatformStorageConnectionError::Unavailable)?;
            if verify_platform_storage_markers(
                &self.config,
                &mut connection,
                &self.original_markers,
            )
            .await
            .is_err()
            {
                // This close shares the database phase's remaining budget. Failed close still
                // returns failure; timeout/cancellation drops state, not proof of remote rollback.
                let _ = connection.close().await;
                return Err(PlatformStorageConnectionError::Unavailable);
            }
            Ok(connection)
        })
        .await
        .map_err(|_| PlatformStorageConnectionError::Unavailable)?
    }
}

async fn close(connection: PgConnection) {
    // Only root-postcheck failure reaches here, outside the already finished database budget.
    // The public result is unavailable regardless of whether graceful close is acknowledged.
    let _ = tokio::time::timeout(Duration::from_secs(10), connection.close()).await;
}
