//! Aggregate, bounded MVCC maintenance reads. Not the locking runtime authorization path.
use super::*;
use crate::services::auth_service::durable_source::reconciliation::{
    budget, CheckResult, ReconciliationError as Error,
};
use std::collections::{HashMap, HashSet};

mod history;
mod records;

pub(crate) struct CheckedRegistry {
    pub workspace_status: WorkspaceStatus,
    pub identities: HashMap<PrincipalId, StoredLegacyIdentity>,
    pub policies: HashMap<PrincipalId, StoredLegacyAccessPolicy>,
    pub identity_audit_entries: u64,
    pub policy_audit_entries: u64,
}
impl CollaborationIdentityStore {
    pub(crate) async fn reconcile_snapshot(
        tx: &mut Transaction<'_, Postgres>,
        scope: WorkspaceRef,
    ) -> CheckResult<CheckedRegistry> {
        // The source owns the snapshot and publication boundary. Reject accidental composition
        // into a mutable/read-committed transaction; do not relax ordinary row-locking readers.
        let valid: bool = sqlx::query(
            "SELECT current_setting('transaction_read_only')='on'
            AND current_setting('transaction_isolation')='repeatable read' AS valid",
        )
        .fetch_one(&mut **tx)
        .await?
        .try_get("valid")?;
        if !valid {
            return Err(Error::StorageUnavailable);
        }
        let schema_error = |error| match error {
            IdentityStoreError::StorageUnavailable => Error::StorageUnavailable,
            _ => Error::UnsupportedSchema,
        };
        Self::verify_audit_schema(tx).await.map_err(schema_error)?;
        Self::verify_identity_audit_schema(tx)
            .await
            .map_err(schema_error)?;
        let row = sqlx::query("SELECT m.workspace_id,
            CASE w.status WHEN 'active' THEN 'active' WHEN 'archived' THEN 'archived' ELSE 'invalid' END AS status
            FROM collaboration_legacy_workspaces m
            JOIN collaboration_workspaces w ON w.authority_id=m.authority_id AND w.workspace_id=m.workspace_id
            JOIN collaboration_authorities a ON a.authority_id=m.authority_id WHERE m.authority_id=$1")
            .bind(scope.authority_id.as_uuid()).fetch_optional(&mut **tx).await?.ok_or(Error::ScopeMismatch)?;
        if row.try_get::<uuid::Uuid, _>("workspace_id")? != scope.workspace_id.as_uuid() {
            return Err(Error::ScopeMismatch);
        }
        let workspace_status = match row.try_get::<&str, _>("status")? {
            "active" => WorkspaceStatus::Active,
            "archived" => WorkspaceStatus::Archived,
            _ => return Err(Error::ScopeMismatch),
        };
        let identities = records::identities(tx, scope).await?;
        let policies = records::policies(tx, scope, &identities).await?;
        let identity_audit_entries = history::identities(tx, scope, &identities).await?;
        let policy_audit_entries = history::policies(tx, scope, &identities, &policies).await?;
        Ok(CheckedRegistry {
            workspace_status,
            identities,
            policies,
            identity_audit_entries,
            policy_audit_entries,
        })
    }
}
