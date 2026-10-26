//! Explicit PostgreSQL + private Unix-file staging; no live composition or volatile fallback.
//! A trusted adapter must authenticate and authorize the supplied Principal. Owner filtering
//! alone is not authentication; the durable-source Session adapter composes these checks separately.
use super::prepared_resource_sql;
use crate::{
    models::collaboration::*,
    sqlx_compat::{self as sqlx, PgPool, Postgres, Row},
};
use sqlx_core::transaction::Transaction;
use std::{future::Future, path::Path, sync::Arc, time::Duration};

mod blob;
mod commands;
mod draft;
#[cfg(test)]
mod draft_tests;
mod reader;
mod records;
mod schema;
mod writer;
pub(crate) use draft::validate_metadata as validate_artifact_metadata;
pub use draft::ArtifactDraft;
pub(crate) use reader::ArtifactContentReader;
pub(crate) use writer::ArtifactContentWriter;
const MAX_CONTENT: usize = 1_048_576;
type Result<T> = std::result::Result<T, ArtifactStoreError>;
type Tx<'a> = Transaction<'a, Postgres>;

/// Namespace/schema pin only; no root opening, content read, publication or commit.
pub(crate) async fn validate_artifact_namespace_in_transaction(
    tx: &mut Tx<'_>,
    scope: WorkspaceRef,
) -> Result<()> {
    schema::check_metadata_schema(scope, tx, true).await
}

fn check_scope(scope: WorkspaceRef, workspace: WorkspaceRef, owner: PrincipalRef) -> Result<()> {
    if workspace != scope || owner.authority_id != scope.authority_id {
        return Err(ArtifactStoreError::ScopeMismatch);
    }
    Ok(())
}

/// Reuse the existing read-only private-root walk without constructing a store or touching SQL.
pub(crate) fn open_private_artifact_root(root: &Path) -> Result<std::fs::File> {
    blob::open_root(root).map_err(|_| ArtifactStoreError::BlobUnavailable)
}

#[derive(Clone)]
pub struct ArtifactStore {
    pool: PgPool,
    scope: WorkspaceRef,
    blobs: Arc<blob::BlobDirectory>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ArtifactStoreError {
    #[error("artifact storage did not confirm the operation")]
    StorageUnavailable,
    #[error("artifact content is unavailable or invalid")]
    BlobUnavailable,
    #[error("unsupported artifact schema")]
    UnsupportedSchema,
    #[error("artifact namespace is not empty")]
    NamespaceNotEmpty,
    #[error("artifact scope does not match")]
    ScopeMismatch,
    #[error("invalid artifact input")]
    InvalidInput,
    #[error("artifact identity or content path already exists")]
    Conflict,
    #[error("artifact not found")]
    NotFound,
    #[error("artifact head revision changed")]
    StaleRevision,
    #[error("invalid persisted artifact record")]
    InvalidRecord,
}
impl From<sqlx::Error> for ArtifactStoreError {
    fn from(error: sqlx::Error) -> Self {
        if error
            .as_database_error()
            .is_some_and(|error| error.is_unique_violation())
        {
            Self::Conflict
        } else {
            Self::StorageUnavailable
        }
    }
}
async fn bounded<T>(future: impl Future<Output = Result<T>>) -> Result<T> {
    // Timeout/cancellation can race commit. Never infer rollback or automatically remove blobs.
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .map_err(|_| ArtifactStoreError::StorageUnavailable)?
}
fn confirmed_one(rows: u64) -> Result<()> {
    if rows == 1 {
        Ok(())
    } else {
        Err(ArtifactStoreError::StorageUnavailable)
    }
}

impl ArtifactStore {
    /// Opens an existing private directory without creating/chmodding it; no database I/O.
    pub fn open(pool: PgPool, scope: WorkspaceRef, root: &Path) -> Result<Self> {
        Ok(Self {
            pool,
            scope,
            blobs: Arc::new(blob::BlobDirectory::open(root)?),
        })
    }
    fn check_scope(&self, workspace: WorkspaceRef, owner: PrincipalRef) -> Result<()> {
        check_scope(self.scope, workspace, owner)
    }
    async fn transaction(&self, readonly: bool) -> Result<Tx<'_>> {
        let mut tx = prepared_resource_sql::begin(&self.pool, readonly).await?;
        Self::namespace(&mut tx).await?;
        Ok(tx)
    }
    async fn read_blob(&self, revision: &ArtifactRevision) -> Result<Vec<u8>> {
        ArtifactContentReader::from_store(self)
            .read_blob(revision)
            .await
    }
    async fn verify_blob_root(&self) -> Result<()> {
        ArtifactContentReader::from_store(self).verify_root().await
    }
    /// Exact revision only. Neither digest knowledge nor shared content bypasses owner filtering.
    pub async fn read(
        &self,
        reference: &ArtifactRef,
        owner: PrincipalRef,
    ) -> Result<Option<ArtifactContent>> {
        self.check_scope(reference.workspace, owner)?;
        bounded(async {
            let mut tx = self.transaction(true).await?;
            let result = ArtifactContentReader::from_store(self)
                .read_content(&mut tx, reference, owner, false)
                .await?;
            tx.commit().await?;
            Ok(result)
        })
        .await
    }

    /// Hold schema/head locks until the trusted caller finishes its authorization transaction.
    pub(crate) async fn read_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        reference: &ArtifactRef,
        owner: PrincipalRef,
    ) -> Result<Option<ArtifactContent>> {
        ArtifactContentReader::from_store(self)
            .read_in_transaction(tx, reference, owner)
            .await
    }

    /// Pin schema metadata even when a trusted composition has no content references to read.
    pub(crate) async fn validate_namespace_in_transaction(&self, tx: &mut Tx<'_>) -> Result<()> {
        ArtifactContentReader::from_store(self)
            .validate_namespace_in_transaction(tx)
            .await
    }
}
