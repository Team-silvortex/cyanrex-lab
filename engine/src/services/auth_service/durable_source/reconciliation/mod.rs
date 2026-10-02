//! Trusted maintenance observation, not authorization, repair, bootstrap proof or secret recovery.
use super::*;
use crate::{
    models::collaboration::{PrincipalStatus, WorkspaceRef, WorkspaceStatus},
    services::collaboration_identity_store::CollaborationIdentityStore as Registry,
};
use std::collections::HashSet;

mod columns;
mod schema;
mod source;
pub(crate) type CheckResult<T> = std::result::Result<T, ReconciliationError>;
pub(crate) const MAX_ROWS: usize = 10_000;
const MAX_COLLECTION_BYTES: i64 = 4 * 1024 * 1024;
const MAX_ROW_BYTES: i64 = 16 * 1024;

/// Aggregate observations at one MVCC snapshot/time. Contains no identities, credentials or tokens.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AuthorityReconciliation {
    pub workspace: WorkspaceRef,
    pub workspace_status: WorkspaceStatus,
    pub observed_at: DateTime<Utc>,
    pub accounts: u64,
    pub unbound_accounts: u64,
    pub retired_accounts_present: u64,
    pub sessions: u64,
    pub expired_sessions: u64,
    pub bound_identities: u64,
    pub retired_identities: u64,
    pub disabled_identities: u64,
    pub policy_records: u64,
    pub current_managers: u64,
    pub identity_audit_entries: u64,
    pub policy_audit_entries: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReconciliationError {
    #[error("reconciliation storage did not confirm a complete snapshot")]
    StorageUnavailable,
    #[error("reconciliation requires supported source and audited registry schemas")]
    UnsupportedSchema,
    #[error("reconciliation requires one explicit non-system namespace")]
    InvalidNamespace,
    #[error("reconciliation scope does not match the durable source and legacy mapping")]
    ScopeMismatch,
    #[error("reconciliation snapshot exceeds the bounded row or metadata budget")]
    LimitExceeded,
    #[error("source account/session coordinates do not reconcile")]
    InvalidSource,
    #[error("identity records do not reconcile")]
    InvalidIdentity,
    #[error("membership/deployment records do not reconcile")]
    InvalidPolicy,
    #[error("identity audit history or current head does not reconcile")]
    IdentityHistoryMismatch,
    #[error("policy audit history or current head does not reconcile")]
    PolicyHistoryMismatch,
    #[error("no current source-backed audited manager was observed")]
    NoCurrentManager,
}
impl ReconciliationError {
    pub fn code(self) -> &'static str {
        match self {
            Self::StorageUnavailable => "reconciliation_unavailable",
            Self::UnsupportedSchema => "reconciliation_unsupported_schema",
            Self::InvalidNamespace => "reconciliation_invalid_namespace",
            Self::ScopeMismatch => "reconciliation_scope_mismatch",
            Self::LimitExceeded => "reconciliation_limit_exceeded",
            Self::InvalidSource => "reconciliation_invalid_source",
            Self::InvalidIdentity => "reconciliation_invalid_identity",
            Self::InvalidPolicy => "reconciliation_invalid_policy",
            Self::IdentityHistoryMismatch => "reconciliation_identity_history_mismatch",
            Self::PolicyHistoryMismatch => "reconciliation_policy_history_mismatch",
            Self::NoCurrentManager => "reconciliation_no_current_manager",
        }
    }
}
impl From<sqlx::Error> for ReconciliationError {
    fn from(_: sqlx::Error) -> Self {
        Self::StorageUnavailable
    }
}

/// All fragments are source-owned constants, never caller-provided SQL. Bound before fetching
/// variable-size records; count/size queries themselves remain under the statement/total deadline.
pub(crate) async fn budget(
    connection: &mut PgConnection,
    table: &str,
    predicate: &str,
    bytes: &str,
    authority: AuthorityId,
) -> CheckResult<u64> {
    let row = sqlx::query(&format!(
        "SELECT count(*) AS rows,
        COALESCE(sum({bytes}), 0)::BIGINT AS bytes, COALESCE(max({bytes}), 0)::BIGINT AS largest
        FROM {table} WHERE {predicate}"
    ))
    .bind(authority.as_uuid())
    .fetch_one(connection)
    .await?;
    let rows: i64 = row.try_get("rows")?;
    if rows > MAX_ROWS as i64
        || row.try_get::<i64, _>("bytes")? > MAX_COLLECTION_BYTES
        || row.try_get::<i64, _>("largest")? > MAX_ROW_BYTES
    {
        return Err(ReconciliationError::LimitExceeded);
    }
    u64::try_from(rows).map_err(|_| ReconciliationError::StorageUnavailable)
}

impl DurableAuthSource {
    /// Read-only, repeatable-read bounded reconciliation of the staged legacy authority. No
    /// row/advisory writer locks, cache, cleanup, installer, repair or public Session authorization.
    /// The view can precede an in-flight commit; successful observation never authorizes a retry.
    pub async fn reconcile_authority(
        &self,
        scope: WorkspaceRef,
    ) -> CheckResult<AuthorityReconciliation> {
        if scope.authority_id != self.authority_id {
            return Err(ReconciliationError::ScopeMismatch);
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.pool.begin().await?;
            sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET LOCAL lock_timeout = '2s'")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET LOCAL statement_timeout = '5s'")
                .execute(&mut *tx)
                .await?;
            let observed_at = schema::verify(&mut tx, scope).await?;
            let source = source::read(&mut tx, scope.authority_id, observed_at).await?;
            let registry = Registry::reconcile_snapshot(&mut tx, scope).await?;
            let mut bound_accounts = HashSet::new();
            let mut retired_accounts_present = 0;
            let mut current_managers = 0;
            for identity in registry.identities.values() {
                if source
                    .accounts
                    .get(&identity.account_id)
                    .is_some_and(|name| name != &identity.binding.username)
                {
                    return Err(ReconciliationError::InvalidSource);
                }
                let current =
                    source.accounts.get(&identity.account_id) == Some(&identity.binding.username);
                if !current && identity.retired_at.is_none() {
                    return Err(ReconciliationError::InvalidSource);
                }
                if current {
                    bound_accounts.insert(identity.account_id);
                    if identity.retired_at.is_some() {
                        retired_accounts_present += 1;
                    } else if identity.principal.status == PrincipalStatus::Active
                        && registry
                            .policies
                            .get(&identity.binding.principal_id)
                            .is_some_and(|p| p.policy.deployment_granted)
                    {
                        current_managers += 1;
                    }
                }
            }
            if current_managers == 0 {
                return Err(ReconciliationError::NoCurrentManager);
            }
            let report = AuthorityReconciliation {
                workspace: scope,
                workspace_status: registry.workspace_status,
                observed_at,
                accounts: source.accounts.len() as u64,
                unbound_accounts: (source.accounts.len() - bound_accounts.len()) as u64,
                retired_accounts_present,
                sessions: source.sessions,
                expired_sessions: source.expired_sessions,
                bound_identities: registry.identities.len() as u64,
                retired_identities: registry
                    .identities
                    .values()
                    .filter(|i| i.retired_at.is_some())
                    .count() as u64,
                disabled_identities: registry
                    .identities
                    .values()
                    .filter(|i| i.principal.status == PrincipalStatus::Disabled)
                    .count() as u64,
                policy_records: registry.policies.len() as u64,
                current_managers,
                identity_audit_entries: registry.identity_audit_entries,
                policy_audit_entries: registry.policy_audit_entries,
            };
            tx.commit().await?;
            Ok(report)
        })
        .await
        .map_err(|_| ReconciliationError::StorageUnavailable)?
    }
}
