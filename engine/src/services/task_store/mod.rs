//! Explicit PostgreSQL-only work staging, never composed into the live application.
//! Caller-supplied Principal/scope must already be authenticated and authorized by a trusted
//! adapter. Owner filtering is not authentication. No automatic schema installation or fallback.
use crate::{
    models::collaboration::*,
    sqlx_compat::{self as sqlx, PgPool, Postgres, Row},
};
use sqlx_core::transaction::Transaction;
use std::{future::Future, time::Duration};

mod commands;
mod draft;
mod records;
mod schema;
pub use draft::TaskDraft;

#[derive(Clone)]
pub struct TaskStore {
    pool: PgPool,
    scope: WorkspaceRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TaskStoreError {
    #[error("task storage did not confirm the operation")]
    StorageUnavailable,
    #[error("unsupported task schema")]
    UnsupportedSchema,
    #[error("task namespace is not empty")]
    NamespaceNotEmpty,
    #[error("task scope does not match")]
    ScopeMismatch,
    #[error("invalid task input")]
    InvalidInput,
    #[error("unsupported task definition")]
    UnknownDefinition,
    #[error("task identifier already exists")]
    Conflict,
    #[error("task not found")]
    NotFound,
    #[error("task revision changed")]
    StaleRevision,
    #[error("task transition is not allowed")]
    InvalidTransition,
    #[error("invalid persisted task record")]
    InvalidRecord,
}
type Result<T> = std::result::Result<T, TaskStoreError>;
type Tx<'a> = Transaction<'a, Postgres>;

impl From<sqlx::Error> for TaskStoreError {
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
    // Timeout/cancellation may race commit; errors never establish rollback.
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .map_err(|_| TaskStoreError::StorageUnavailable)?
}
fn confirmed_one(rows: u64) -> Result<()> {
    if rows != 1 {
        return Err(TaskStoreError::StorageUnavailable);
    }
    Ok(())
}

impl TaskStore {
    pub fn new(pool: PgPool, scope: WorkspaceRef) -> Self {
        Self { pool, scope }
    }

    fn check_scope(&self, reference: TaskRef, owner: PrincipalRef) -> Result<()> {
        if reference.workspace != self.scope || owner.authority_id != self.scope.authority_id {
            return Err(TaskStoreError::ScopeMismatch);
        }
        Ok(())
    }

    async fn transaction(&self, readonly: bool) -> Result<Tx<'_>> {
        let mut tx = self.pool.begin().await?;
        if readonly {
            sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("SET LOCAL statement_timeout = '5s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL lock_timeout = '2s'")
            .execute(&mut *tx)
            .await?;
        Self::namespace(&mut tx).await?;
        Ok(tx)
    }

    pub async fn get(
        &self,
        reference: TaskRef,
        owner: PrincipalRef,
    ) -> Result<Option<TaskSnapshot>> {
        self.check_scope(reference, owner)?;
        bounded(async {
            let mut tx = self.transaction(true).await?;
            self.check_schema(&mut tx, false).await?;
            let snapshot = self.read(&mut tx, reference, owner, false).await?;
            tx.commit().await?;
            Ok(snapshot)
        })
        .await
    }

    /// Keep schema and task locks within the trusted caller's transaction until it finishes.
    pub(crate) async fn get_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        reference: TaskRef,
        owner: PrincipalRef,
    ) -> Result<Option<TaskSnapshot>> {
        self.check_scope(reference, owner)?;
        self.check_schema(tx, true).await?;
        self.read(tx, reference, owner, true).await
    }
}
