//! Explicit intent staging and per-step journaling; no public dispatch or recovery grant.
use super::prepared_resource_sql;
use crate::{
    models::collaboration::{LegacyAccountId, PrincipalRef, TaskRef, WorkspaceRef},
    sqlx_compat::{self as sqlx, PgPool, Postgres, Row},
};
use sqlx_core::transaction::Transaction;
use uuid::Uuid;

mod dispatch;
mod inspection;
mod records;
mod schema;
pub(crate) use schema::IntentRelations;
type Result<T> = std::result::Result<T, DraftIntentJournalError>;
type Tx<'a> = Transaction<'a, Postgres>;

/// Internal command data only; public callers cannot supply an owner or imported checkpoint.
#[derive(PartialEq, Eq)]
pub(crate) struct DraftIntentRecord {
    pub task: TaskRef,
    pub owner: PrincipalRef,
    pub account_id: LegacyAccountId,
    pub source_namespace: String,
    pub task_namespace: String,
    pub artifact_namespace: String,
    pub checkpoint: Vec<u8>,
}

/// A step coordinate, not a reusable authorization or permission to retry an unknown write.
#[derive(PartialEq, Eq)]
pub(crate) struct DraftDispatchRecord {
    pub task: TaskRef,
    pub ordinal: usize,
    pub nonce: Uuid,
    pub committed: bool,
}

#[derive(Clone)]
pub struct DraftIntentJournal {
    pub(crate) pool: PgPool,
    pub(crate) scope: WorkspaceRef,
    pub(crate) installation_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DraftIntentJournalError {
    #[error("draft intent storage did not confirm the operation")]
    StorageUnavailable,
    #[error("unsupported draft intent schema")]
    UnsupportedSchema,
    #[error("draft intent namespace is not empty")]
    NamespaceNotEmpty,
    #[error("draft intent scope does not match")]
    ScopeMismatch,
    #[error("draft intent journal installation does not match")]
    IncarnationMismatch,
    #[error("invalid draft intent input")]
    InvalidInput,
    #[error("invalid persisted draft intent")]
    InvalidRecord,
    #[error("draft intent target already exists")]
    Conflict,
}
impl From<sqlx::Error> for DraftIntentJournalError {
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
impl DraftIntentJournal {
    /// Trusted fixed configuration only. No connect, install or automatic identity adoption.
    pub fn new(pool: PgPool, scope: WorkspaceRef, installation_id: Uuid) -> Result<Self> {
        if installation_id.get_version() != Some(uuid::Version::Random)
            || installation_id.get_variant() != uuid::Variant::RFC4122
        {
            return Err(DraftIntentJournalError::InvalidInput);
        }
        Ok(Self {
            pool,
            scope,
            installation_id,
        })
    }
}

async fn bounded<T>(operation: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    // Commit may race timeout or cancellation; failure never establishes absence/rollback.
    tokio::time::timeout(std::time::Duration::from_secs(10), operation)
        .await
        .map_err(|_| DraftIntentJournalError::StorageUnavailable)?
}

fn confirmed_one(rows: u64) -> Result<()> {
    if rows == 1 {
        Ok(())
    } else {
        Err(DraftIntentJournalError::StorageUnavailable)
    }
}
