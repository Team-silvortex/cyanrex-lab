//! Explicit, PostgreSQL-only C1 identity, lifecycle audit and legacy access staging. Not composed into AppState
//! or the live AuthService. The explicit durable auth source can compose session-authorized commands
//! on one transaction. Stored policies and their previews are not credentials or live route guards.
//! All methods fail closed on storage errors, without caches, file fallback or environment reads.
//! The trusted caller supplies verified scope and a stable account-incarnation ID. A username,
//! credential digest, import timestamp or client claim cannot establish that incarnation.

use std::{future::Future, time::Duration};

use chrono::{DateTime, Utc};
use sqlx_core::transaction::Transaction;
use sqlx_postgres::PgConnection;

use crate::{
    models::collaboration::*,
    sqlx_compat::{self as sqlx, PgPool, Postgres, Row},
};

mod access;
mod access_policy;
mod access_schema;
mod access_write;
mod accounts;
mod bootstrap;
mod identity_audit;
mod identity_audit_entry;
mod identity_audit_schema;
mod identity_command;
mod identity_write;
mod policy_audit;
mod policy_audit_schema;
mod policy_command;
mod reconciliation;
mod schema;
mod session_adapter;

pub use access::{LegacyAccessPolicy, StoredLegacyAccessPolicy};
pub use access_policy::LegacyPolicyResource;
pub use identity_audit_entry::{
    LegacyIdentityAuditEntry, LegacyIdentityAuditKind, LegacyIdentityReceipt,
};
pub use identity_command::{LegacyIdentityAction, LegacyIdentityCommand};
pub use policy_audit::{LegacyPolicyAuditEntry, LegacyPolicyReceipt, PolicyAuditKind};
pub use policy_command::LegacyPolicyCommand;

#[derive(Clone)]
pub struct CollaborationIdentityStore {
    pool: PgPool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredLegacyIdentity {
    pub binding: LegacyIdentityBinding,
    pub account_id: LegacyAccountId,
    pub principal: Principal,
    pub retired_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IdentityStoreError {
    #[error("identity storage did not confirm the operation")]
    StorageUnavailable,
    #[error("unsupported identity schema version")]
    UnsupportedSchema,
    #[error("legacy workspace is not provisioned")]
    ScopeNotFound,
    #[error("legacy workspace mapping does not match")]
    ScopeConflict,
    #[error("legacy workspace is not active")]
    ScopeInactive,
    #[error("identity mapping conflicts with an existing account")]
    IdentityConflict,
    #[error("account incarnation is retired")]
    IdentityRetired,
    #[error("principal is disabled")]
    IdentityInactive,
    #[error("identity no longer matches the expected principal")]
    StaleIdentity,
    #[error("access policy no longer matches the reviewed revision")]
    StaleAccess,
    #[error("policy audit schema has not been explicitly enabled")]
    AuditNotEnabled,
    #[error("policy changes require an attributed command")]
    AuditContextRequired,
    #[error("identity audit schema has not been explicitly enabled")]
    IdentityAuditNotEnabled,
    #[error("identity changes require an attributed lifecycle command")]
    IdentityAuditContextRequired,
    #[error("each legacy authority requires a verified active manager before audit activation")]
    AuditBootstrapRequired,
    #[error("current principal is not authorized to manage instance policy")]
    AccessDenied,
    #[error("command ID is already bound to a different request")]
    CommandConflict,
    #[error("operation would remove the last active instance manager")]
    LastAuthorityManager,
    #[error("invalid persisted identity record")]
    InvalidRecord,
}

type Result<T> = std::result::Result<T, IdentityStoreError>;

impl From<sqlx::Error> for IdentityStoreError {
    fn from(error: sqlx::Error) -> Self {
        if error
            .as_database_error()
            .is_some_and(|error| error.is_unique_violation())
        {
            Self::IdentityConflict
        } else {
            Self::StorageUnavailable
        }
    }
}

impl From<ContractError> for IdentityStoreError {
    fn from(_: ContractError) -> Self {
        Self::InvalidRecord
    }
}

async fn bounded<T>(operation: impl Future<Output = Result<T>>) -> Result<T> {
    // Caller cancellation/expiry can race commit: an error is never a promise of rollback.
    tokio::time::timeout(Duration::from_secs(10), operation)
        .await
        .map_err(|_| IdentityStoreError::StorageUnavailable)?
}

fn confirmed_one(rows: u64) -> Result<()> {
    if rows != 1 {
        return Err(IdentityStoreError::StorageUnavailable);
    }
    Ok(())
}

impl CollaborationIdentityStore {
    /// No DDL, allocation, connection or legacy data access is performed by construction.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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

    async fn version(connection: &mut PgConnection) -> Result<i32> {
        let row = sqlx::query(
            "SELECT version FROM collaboration_identity_schema WHERE singleton FOR SHARE",
        )
        .fetch_one(connection)
        .await?;
        let version = row.try_get::<i32, _>("version")?;
        if !matches!(version, 1 | 2) {
            return Err(IdentityStoreError::UnsupportedSchema);
        }
        Ok(version)
    }

    async fn workspace(
        connection: &mut PgConnection,
        authority: AuthorityId,
    ) -> Result<Option<Workspace>> {
        let row = sqlx::query(
            "SELECT m.authority_id, m.workspace_id, w.title, w.status
            FROM collaboration_legacy_workspaces m JOIN collaboration_workspaces w
            ON w.authority_id = m.authority_id AND w.workspace_id = m.workspace_id
            WHERE m.authority_id = $1 FOR SHARE OF m, w",
        )
        .bind(authority.as_uuid())
        .fetch_optional(connection)
        .await?;
        row.map(|row| {
            let status = match row.try_get::<&str, _>("status")? {
                "active" => WorkspaceStatus::Active,
                "archived" => WorkspaceStatus::Archived,
                _ => return Err(IdentityStoreError::InvalidRecord),
            };
            Ok(Workspace {
                reference: WorkspaceRef {
                    authority_id: AuthorityId::try_from(
                        row.try_get::<uuid::Uuid, _>("authority_id")?,
                    )?,
                    workspace_id: WorkspaceId::try_from(
                        row.try_get::<uuid::Uuid, _>("workspace_id")?,
                    )?,
                },
                title: row.try_get("title")?,
                status,
            })
        })
        .transpose()
    }

    async fn scope(
        connection: &mut PgConnection,
        expected: WorkspaceRef,
        writer: bool,
    ) -> Result<Workspace> {
        if writer {
            // First iteration serializes identity mutations per authority, including empty-key races.
            // All independent pools/processes use this DB row lock, not an in-process mutex.
            sqlx::query("SELECT authority_id FROM collaboration_authorities WHERE authority_id = $1 FOR UPDATE")
                .bind(expected.authority_id.as_uuid()).fetch_optional(&mut *connection).await?
                .ok_or(IdentityStoreError::ScopeNotFound)?;
        }
        let workspace = Self::workspace(connection, expected.authority_id)
            .await?
            .ok_or(IdentityStoreError::ScopeNotFound)?;
        if workspace.reference != expected {
            return Err(IdentityStoreError::ScopeConflict);
        }
        Ok(workspace)
    }

    pub async fn legacy_workspace(&self, authority: AuthorityId) -> Result<Option<Workspace>> {
        bounded(async {
            let mut tx = self.transaction().await?;
            Self::version(&mut tx).await?;
            let workspace = Self::workspace(&mut tx, authority).await?;
            tx.commit().await?;
            Ok(workspace)
        })
        .await
    }

    /// Pre-audit provisioning only; identity schema 2 fences this maintenance writer.
    /// With schema 1, repeating the same operator-pinned mapping is safe;
    /// a different workspace ID is rejected, never silently adopted or created alongside it.
    pub async fn provision_legacy_workspace(&self, expected: WorkspaceRef) -> Result<Workspace> {
        bounded(async {
            let mut tx = self.transaction().await?;
            if Self::version(&mut tx).await? != 1 { return Err(IdentityStoreError::IdentityAuditContextRequired); }
            sqlx::query("INSERT INTO collaboration_authorities (authority_id) VALUES ($1) ON CONFLICT DO NOTHING")
                .bind(expected.authority_id.as_uuid()).execute(&mut *tx).await?;
            sqlx::query("SELECT authority_id FROM collaboration_authorities WHERE authority_id = $1 FOR UPDATE")
                .bind(expected.authority_id.as_uuid()).fetch_one(&mut *tx).await?;
            if let Some(workspace) = Self::workspace(&mut tx, expected.authority_id).await? {
                if workspace.reference != expected { return Err(IdentityStoreError::ScopeConflict); }
                if workspace.status != WorkspaceStatus::Active { return Err(IdentityStoreError::ScopeInactive); }
                tx.commit().await?;
                return Ok(workspace);
            }
            confirmed_one(sqlx::query("INSERT INTO collaboration_workspaces (authority_id, workspace_id, title, status)
                VALUES ($1, $2, 'Legacy teaching workspace', 'active')")
                .bind(expected.authority_id.as_uuid()).bind(expected.workspace_id.as_uuid())
                .execute(&mut *tx).await?.rows_affected())?;
            confirmed_one(sqlx::query("INSERT INTO collaboration_legacy_workspaces (authority_id, workspace_id) VALUES ($1, $2)")
                .bind(expected.authority_id.as_uuid()).bind(expected.workspace_id.as_uuid())
                .execute(&mut *tx).await?.rows_affected())?;
            let workspace = Self::scope(&mut tx, expected, false).await?;
            tx.commit().await?;
            Ok(workspace)
        }).await
    }
}
