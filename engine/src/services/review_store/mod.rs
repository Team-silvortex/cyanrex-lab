//! Explicit PostgreSQL staging of attributed judgments, with no live composition or fallback.
//! Caller-supplied reviewer/source is trusted adapter input, NOT authentication. Future public
//! commands must compose Session, grants, resource/evidence checks and mutation atomically.
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
pub use draft::{HumanReviewEdit, ReviewDraft};
type Result<T> = std::result::Result<T, ReviewStoreError>;
type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Clone)]
pub struct ReviewStore {
    pool: PgPool,
    scope: WorkspaceRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReviewStoreError {
    #[error("review storage did not confirm the operation")]
    StorageUnavailable,
    #[error("unsupported review schema")]
    UnsupportedSchema,
    #[error("review namespace is not empty")]
    NamespaceNotEmpty,
    #[error("review scope does not match")]
    ScopeMismatch,
    #[error("invalid review input")]
    InvalidInput,
    #[error("review identifier already exists")]
    Conflict,
    #[error("review not found")]
    NotFound,
    #[error("review revision changed")]
    StaleRevision,
    #[error("only a human judgment can be amended")]
    NotAmendable,
    #[error("invalid persisted review record")]
    InvalidRecord,
}
impl From<sqlx::Error> for ReviewStoreError {
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
    // Cancellation or lost commit acknowledgement is not evidence of rollback.
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .map_err(|_| ReviewStoreError::StorageUnavailable)?
}
fn confirmed_one(rows: u64) -> Result<()> {
    if rows == 1 {
        Ok(())
    } else {
        Err(ReviewStoreError::StorageUnavailable)
    }
}

impl ReviewStore {
    /// No I/O, environment configuration or implicit installation.
    pub fn new(pool: PgPool, scope: WorkspaceRef) -> Self {
        Self { pool, scope }
    }

    fn check_scope(&self, workspace: WorkspaceRef, reviewer: PrincipalRef) -> Result<()> {
        if workspace != self.scope || reviewer.authority_id != self.scope.authority_id {
            return Err(ReviewStoreError::ScopeMismatch);
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
    /// Exact history read. The returned opinion may be superseded and is not a current approval.
    pub async fn read(
        &self,
        reference: ReviewRef,
        reviewer: PrincipalRef,
    ) -> Result<Option<ReviewRecord>> {
        self.check_scope(reference.workspace, reviewer)?;
        bounded(async {
            let mut tx = self.transaction(true).await?;
            self.check_schema(&mut tx, false).await?;
            let result = match self
                .head(&mut tx, reference.review_id, reviewer, false)
                .await?
            {
                None => None,
                Some(head)
                    if u64::from(reference.revision) > u64::from(head.reference.revision) =>
                {
                    None
                }
                Some(head) if head.reference == reference => Some(head),
                Some(head) => {
                    let historical = self.record(&mut tx, reference, reviewer).await?;
                    if !records::same_subject(&head, &historical) {
                        return Err(ReviewStoreError::InvalidRecord);
                    }
                    Some(historical)
                }
            };
            tx.commit().await?;
            Ok(result)
        })
        .await
    }
}
