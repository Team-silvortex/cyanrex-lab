//! Explicit trusted recipient metadata for one exact file version, not download authorization.
//!
//! Construction is inert. Issuer, file publisher and recipient are independent exact accounts.
//! Issuer/actor attribution is caller-supplied, not authentication or an audited permission check.
//! Expired/revoked grants retain slots. Lost acknowledgement can follow a committed mutation and
//! never permits retry, adoption, cleanup or automatic recovery. No publisher grant is consulted.
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
pub(crate) use mutation::PendingFileRecipientGrantMutation;
pub(crate) use transaction::PendingFileRecipientGrantRead;

pub const MAX_FILE_RECIPIENT_GRANTS: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FileRecipientGrantStoreError {
    #[error("recipient grant storage did not confirm the operation")]
    StorageUnavailable,
    #[error("unsupported recipient grant schema")]
    UnsupportedSchema,
    #[error("recipient grant namespace is not empty")]
    NamespaceNotEmpty,
    #[error("recipient grant scope does not match")]
    ScopeMismatch,
    #[error("recipient grant installation does not match")]
    IncarnationMismatch,
    #[error("invalid recipient grant input")]
    InvalidInput,
    #[error("invalid persisted recipient grant")]
    InvalidRecord,
    #[error("recipient grant transition conflicts")]
    Conflict,
    #[error("recipient grant quota exceeded")]
    QuotaExceeded,
}

type Result<T> = std::result::Result<T, FileRecipientGrantStoreError>;
type Tx<'a> = Transaction<'a, Postgres>;

impl From<sqlx::Error> for FileRecipientGrantStoreError {
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
pub struct StoredFileRecipientGrant {
    grant: FileRecipientGrant,
    issuer: FileAccountRef,
}

impl StoredFileRecipientGrant {
    pub fn grant(&self) -> &FileRecipientGrant {
        &self.grant
    }

    pub fn issuer(&self) -> FileAccountRef {
        self.issuer
    }
}

#[derive(Clone)]
pub struct FileRecipientGrantStore {
    pool: PgPool,
    scope: WorkspaceRef,
    installation_id: Uuid,
    namespace: String,
    max_grants: u32,
}

impl FileRecipientGrantStore {
    /// Freeze explicit configuration without connecting, installing or adopting ambient state.
    pub fn new(
        pool: PgPool,
        scope: WorkspaceRef,
        installation_id: Uuid,
        namespace: impl Into<String>,
        max_grants: u32,
    ) -> Result<Self> {
        let namespace = namespace.into();
        if !(1..=MAX_FILE_RECIPIENT_GRANTS).contains(&max_grants)
            || installation_id.get_version() != Some(uuid::Version::Random)
            || installation_id.get_variant() != uuid::Variant::RFC4122
            || !namespace::valid_name(&namespace)
        {
            return Err(FileRecipientGrantStoreError::InvalidInput);
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
            return Err(FileRecipientGrantStoreError::ScopeMismatch);
        }
        Ok(())
    }
}

async fn bounded<T>(operation: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    tokio::time::timeout(std::time::Duration::from_secs(10), operation)
        .await
        .map_err(|_| FileRecipientGrantStoreError::StorageUnavailable)?
}

fn confirmed_one(rows: u64) -> Result<()> {
    if rows != 1 {
        return Err(FileRecipientGrantStoreError::InvalidRecord);
    }
    Ok(())
}
