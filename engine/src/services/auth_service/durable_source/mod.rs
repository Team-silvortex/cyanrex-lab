//! Explicit PostgreSQL authentication-source staging, not composed into AuthService or AppState.
//! Only an empty legacy auth schema can be activated; registration assigns no roles or Principal.
//! Separately initialized audited registries support explicit session-authorized staging commands.
//! No import, public registration/route, environment switch or memory credential/session fallback.
//! Account references and session snapshots are not reusable authorization for later commands.

use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use sqlx_core::transaction::Transaction;
use sqlx_postgres::PgConnection;
use uuid::Uuid;

use super::{
    build_otpauth_uri, derive_password_hash_async, generate_password_salt, generate_totp_secret,
    hash_session_token, same_credentials, verify_password_async, verify_totp, RegisterOk,
    UserRecord,
};
use crate::{
    models::collaboration::{AuthorityId, ContractError, LegacyAccountId, LegacyUsername},
    sqlx_compat::{self as sqlx, PgPool, Postgres, Row},
};

mod accounts;
#[cfg(unix)]
mod artifact_commands;
mod bootstrap;
mod credentials;
mod deletion;
mod private_work;
pub(crate) mod reconciliation;
#[cfg(unix)]
mod review_commands;
mod schema;
mod session_commands;
mod sessions;
mod task_commands;

#[cfg(unix)]
pub use artifact_commands::{SessionArtifactError, SessionArtifactWorkspace};
pub use bootstrap::{AuthorityBootstrap, BootstrapError};
pub use deletion::SessionDeleteAccountCommand;
pub use reconciliation::{AuthorityReconciliation, ReconciliationError};
#[cfg(unix)]
pub use review_commands::{SessionReviewError, SessionReviewWorkspace};
pub use session_commands::{SessionBindCommand, SessionCommandError, SessionPolicyCommand};
#[cfg(unix)]
pub use task_commands::{
    SessionCatalogTaskWorkspace, SessionTaskInputs, SessionTaskInputsWorkspace,
};
pub use task_commands::{SessionTaskError, SessionTaskWorkspace};

#[derive(Clone)]
pub struct DurableAuthSource {
    pool: PgPool,
    authority_id: AuthorityId,
    attempts: Arc<Mutex<HashMap<String, (u8, Instant)>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableAccountRef {
    pub authority_id: AuthorityId,
    pub username: LegacyUsername,
    pub account_id: LegacyAccountId,
}

/// Read-time authentication snapshot only; revocation can happen immediately after return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableSession {
    pub account: DurableAccountRef,
    pub expires_at: DateTime<Utc>,
}

// Secrets intentionally have no Debug or Serialize implementation.
pub struct DurableRegistration {
    pub account: DurableAccountRef,
    pub bootstrap: RegisterOk,
}
pub struct DurableLogin {
    pub token: String,
    pub session: DurableSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DurableAuthError {
    #[error("durable authentication storage did not confirm the operation")]
    StorageUnavailable,
    #[error("unsupported durable authentication source schema")]
    UnsupportedSchema,
    #[error("durable authentication source authority does not match")]
    SourceMismatch,
    #[error(
        "existing accounts or sessions require reviewed migration, not empty-source installation"
    )]
    LegacyDataPresent,
    #[error("invalid durable authentication record")]
    InvalidRecord,
    #[error("invalid authentication input")]
    InvalidInput,
    #[error("account already exists")]
    AccountExists,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("authentication attempts are temporarily limited")]
    RateLimited,
}
type Result<T> = std::result::Result<T, DurableAuthError>;
impl From<sqlx::Error> for DurableAuthError {
    fn from(_: sqlx::Error) -> Self {
        Self::StorageUnavailable
    }
}
impl From<ContractError> for DurableAuthError {
    fn from(_: ContractError) -> Self {
        Self::InvalidRecord
    }
}
async fn bounded<T>(operation: impl Future<Output = Result<T>>) -> Result<T> {
    // Cancellation/timeout can race commit; failure never promises rollback.
    tokio::time::timeout(Duration::from_secs(10), operation)
        .await
        .map_err(|_| DurableAuthError::StorageUnavailable)?
}
fn confirmed_one(rows: u64) -> Result<()> {
    if rows == 1 {
        Ok(())
    } else {
        Err(DurableAuthError::StorageUnavailable)
    }
}
fn valid_token(token: &str) -> bool {
    token.len() == 36
        && Uuid::parse_str(token).is_ok_and(|id| !id.is_nil() && id.to_string() == token)
}

impl DurableAuthSource {
    /// Does not connect, seed an owner, install tables or read environment variables.
    pub fn new(pool: PgPool, authority_id: AuthorityId) -> Self {
        Self {
            pool,
            authority_id,
            attempts: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    async fn transaction(&self) -> Result<Transaction<'_, Postgres>> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET LOCAL lock_timeout = '2s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL statement_timeout = '5s'")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
    async fn lock_source(&self, connection: &mut PgConnection, writer: bool) -> Result<()> {
        // Coherent account/session snapshots, including absent rows, across independent pools.
        let query = if writer {
            "SELECT version, authority_id FROM collaboration_auth_source_schema WHERE singleton FOR UPDATE"
        } else {
            "SELECT version, authority_id FROM collaboration_auth_source_schema WHERE singleton FOR SHARE"
        };
        let row = sqlx::query(query).fetch_one(connection).await?;
        if row.try_get::<i32, _>("version")? != 1 {
            return Err(DurableAuthError::UnsupportedSchema);
        }
        if AuthorityId::try_from(row.try_get::<Uuid, _>("authority_id")?)? != self.authority_id {
            return Err(DurableAuthError::SourceMismatch);
        }
        Ok(())
    }
    async fn now(connection: &mut PgConnection) -> Result<DateTime<Utc>> {
        // Fresh database time after lock waits; transaction-start timestamps can already be stale.
        Ok(sqlx::query("SELECT clock_timestamp() AS now")
            .fetch_one(connection)
            .await?
            .try_get("now")?)
    }
    fn admit_login(&self, username: &LegacyUsername) -> Result<()> {
        let mut attempts = self
            .attempts
            .lock()
            .map_err(|_| DurableAuthError::StorageUnavailable)?;
        let window = Duration::from_secs(300);
        attempts.retain(|_, (_, at)| at.elapsed() < window);
        if !attempts.contains_key(username.as_str()) && attempts.len() >= 1024 {
            return Err(DurableAuthError::RateLimited);
        }
        let (count, _) = attempts
            .entry(username.to_string())
            .or_insert((0, Instant::now()));
        if *count >= 5 {
            return Err(DurableAuthError::RateLimited);
        }
        *count += 1;
        Ok(())
    }
    fn clear_login_attempts(&self, username: &LegacyUsername) {
        if let Ok(mut attempts) = self.attempts.lock() {
            attempts.remove(username.as_str());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn login_admission_is_bounded_and_shared_by_clones() {
        let pool = sqlx::PgPoolOptions::new()
            .connect_lazy("postgres://synthetic@127.0.0.1/unused")
            .unwrap();
        let source = DurableAuthSource::new(pool, AuthorityId::try_from(Uuid::new_v4()).unwrap());
        for number in 0..1024 {
            source
                .admit_login(&format!("synthetic-{number}").parse().unwrap())
                .unwrap();
        }
        let clone = source.clone();
        assert_eq!(
            clone.admit_login(&"overflow-user".parse().unwrap()),
            Err(DurableAuthError::RateLimited)
        );
        let user = "synthetic-0".parse().unwrap();
        for _ in 0..4 {
            clone.admit_login(&user).unwrap();
        }
        assert_eq!(
            source.admit_login(&user),
            Err(DurableAuthError::RateLimited)
        );
        source.clear_login_attempts(&user);
        clone
            .admit_login(&"overflow-user".parse().unwrap())
            .unwrap();
    }
}
