//! Explicit trusted publisher permission metadata, not authentication or an audited issuer.
//!
//! Construction is inert. A recorded issuer/actor is caller-supplied attribution, never authority.
//! Expired and revoked grants retain their slots. Lost acknowledgement may follow a committed
//! write; uncertainty does not prove rollback or allow retry, adoption, repair or cleanup.
use super::prepared_resource_sql;
use crate::{
    models::collaboration::*,
    sqlx_compat::{self as sqlx, PgPool, Postgres, Row},
};
use sqlx_core::transaction::Transaction;
use uuid::Uuid;

mod commands;
mod ledger;
mod mutation;
mod namespace;
mod records;
mod schema;
mod shape;
mod transaction;
pub(crate) use mutation::PendingFilePublisherGrantMutation;
pub(crate) use transaction::PendingFilePublisherGrantRead;

pub const MAX_FILE_PUBLISHER_GRANTS: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FilePublisherGrantStoreError {
    #[error("publisher grant storage did not confirm the operation")]
    StorageUnavailable,
    #[error("unsupported publisher grant schema")]
    UnsupportedSchema,
    #[error("publisher grant namespace is not empty")]
    NamespaceNotEmpty,
    #[error("publisher grant scope does not match")]
    ScopeMismatch,
    #[error("publisher grant installation does not match")]
    IncarnationMismatch,
    #[error("invalid publisher grant input")]
    InvalidInput,
    #[error("invalid persisted publisher grant")]
    InvalidRecord,
    #[error("publisher grant transition conflicts")]
    Conflict,
    #[error("publisher grant quota exceeded")]
    QuotaExceeded,
}

type Result<T> = std::result::Result<T, FilePublisherGrantStoreError>;
type Tx<'a> = Transaction<'a, Postgres>;

impl From<sqlx::Error> for FilePublisherGrantStoreError {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFilePublisherGrant {
    grant: FilePublisherGrant,
    issuer: FileAccountRef,
}

impl StoredFilePublisherGrant {
    pub fn grant(&self) -> &FilePublisherGrant {
        &self.grant
    }

    pub fn issuer(&self) -> FileAccountRef {
        self.issuer
    }
}

#[derive(Clone)]
pub struct FilePublisherGrantStore {
    pool: PgPool,
    scope: WorkspaceRef,
    installation_id: Uuid,
    namespace: String,
    max_grants: u32,
}

impl FilePublisherGrantStore {
    /// Freeze explicit configuration without connecting, installing or adopting ambient state.
    pub fn new(
        pool: PgPool,
        scope: WorkspaceRef,
        installation_id: Uuid,
        namespace: impl Into<String>,
        max_grants: u32,
    ) -> Result<Self> {
        let namespace = namespace.into();
        if !(1..=MAX_FILE_PUBLISHER_GRANTS).contains(&max_grants)
            || installation_id.get_version() != Some(uuid::Version::Random)
            || installation_id.get_variant() != uuid::Variant::RFC4122
            || !namespace::valid_name(&namespace)
        {
            return Err(FilePublisherGrantStoreError::InvalidInput);
        }
        Ok(Self {
            pool,
            scope,
            installation_id,
            namespace,
            max_grants,
        })
    }

    pub(crate) fn namespace(&self) -> &str {
        &self.namespace
    }

    pub(crate) fn scope(&self) -> WorkspaceRef {
        self.scope
    }

    fn check_account(&self, account: FileAccountRef) -> Result<()> {
        if account.principal().authority_id != self.scope.authority_id {
            return Err(FilePublisherGrantStoreError::ScopeMismatch);
        }
        Ok(())
    }
}

async fn bounded<T>(operation: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(std::time::Duration::from_secs(10), operation)
        .await
        .map_err(|_| FilePublisherGrantStoreError::StorageUnavailable)?
}

fn confirmed_one(rows: u64) -> Result<()> {
    if rows != 1 {
        return Err(FilePublisherGrantStoreError::InvalidRecord);
    }
    Ok(())
}
