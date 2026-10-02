//! Borrowed-transaction helpers for fresh-namespace operator bootstrap only.
use super::*;

impl CollaborationIdentityStore {
    /// Caller owns all three installer advisory fences and newly created source writer metadata.
    /// Never use on an existing namespace. No source access or commit occurs inside this helper.
    pub(crate) async fn begin_empty_bootstrap(
        tx: &mut Transaction<'_, Postgres>,
        scope: WorkspaceRef,
    ) -> Result<Workspace> {
        for template in [
            include_str!("../../../migrations/0006_collaboration_identity.sql"),
            include_str!("../../../migrations/0007_collaboration_access.sql"),
        ] {
            for statement in template.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(statement).execute(&mut **tx).await?;
            }
        }
        for table in [
            "collaboration_identity_schema",
            "collaboration_access_schema",
        ] {
            let version: i32 = sqlx::query(&format!(
                "SELECT version FROM {table} WHERE singleton FOR UPDATE"
            ))
            .fetch_one(&mut **tx)
            .await?
            .try_get("version")?;
            if version != 1 {
                return Err(IdentityStoreError::UnsupportedSchema);
            }
        }
        confirmed_one(
            sqlx::query("INSERT INTO collaboration_authorities (authority_id) VALUES ($1)")
                .bind(scope.authority_id.as_uuid())
                .execute(&mut **tx)
                .await?
                .rows_affected(),
        )?;
        sqlx::query(
            "SELECT authority_id FROM collaboration_authorities WHERE authority_id = $1 FOR UPDATE",
        )
        .bind(scope.authority_id.as_uuid())
        .fetch_one(&mut **tx)
        .await?;
        let expected = Workspace {
            reference: scope,
            title: "Legacy teaching workspace".to_string(),
            status: WorkspaceStatus::Active,
        };
        confirmed_one(sqlx::query("INSERT INTO collaboration_workspaces (authority_id, workspace_id, title, status) VALUES ($1, $2, $3, 'active')")
            .bind(scope.authority_id.as_uuid()).bind(scope.workspace_id.as_uuid()).bind(&expected.title)
            .execute(&mut **tx).await?.rows_affected())?;
        confirmed_one(sqlx::query("INSERT INTO collaboration_legacy_workspaces (authority_id, workspace_id) VALUES ($1, $2)")
            .bind(scope.authority_id.as_uuid()).bind(scope.workspace_id.as_uuid()).execute(&mut **tx).await?.rows_affected())?;
        if Self::scope(tx, scope, false).await? != expected {
            return Err(IdentityStoreError::StorageUnavailable);
        }
        Ok(expected)
    }

    /// Return only pending state; the source owner must recheck its account and confirm COMMIT.
    pub(crate) async fn finish_empty_bootstrap(
        tx: &mut Transaction<'_, Postgres>,
        workspace: &Workspace,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
    ) -> Result<(StoredLegacyIdentity, StoredLegacyAccessPolicy)> {
        let scope = workspace.reference;
        let (prior_identity, identity) =
            Self::bind_identity_record(tx, scope, username, account_id).await?;
        if prior_identity.is_some() {
            return Err(IdentityStoreError::IdentityConflict);
        }
        let desired = LegacyAccessPolicy {
            membership: Membership {
                workspace: scope,
                principal_id: identity.binding.principal_id,
                role_refs: vec![
                    "cyanrex.workspace.owner".parse()?,
                    "cyanrex.teaching.teacher".parse()?,
                ],
                status: MembershipStatus::Active,
            },
            deployment_granted: true,
        }
        .canonical()?;
        let (prior_policy, policy) =
            Self::replace_access_record(tx, workspace, None, &desired).await?;
        if prior_policy.is_some() {
            return Err(IdentityStoreError::StaleAccess);
        }
        for template in [
            include_str!("../../../migrations/0008_collaboration_policy_audit.sql"),
            include_str!("../../../migrations/0009_collaboration_identity_audit.sql"),
        ] {
            for statement in template
                .split("-- cyanrex-statement")
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                sqlx::query(statement).execute(&mut **tx).await?;
            }
        }
        // An operator is not an authenticated Principal. Record observed baselines, never invent
        // a historical actor/command ID or claim a retrievable bootstrap receipt.
        Self::append_policy_audit(tx, None, None, &policy).await?;
        Self::append_identity_audit(tx, scope, None, None, &identity).await?;
        for table in [
            "collaboration_access_schema",
            "collaboration_identity_schema",
        ] {
            confirmed_one(
                sqlx::query(&format!(
                    "UPDATE {table} SET version = 2 WHERE singleton AND version = 1"
                ))
                .execute(&mut **tx)
                .await?
                .rows_affected(),
            )?;
        }
        Self::require_policy_audit(tx).await?;
        Self::require_identity_audit(tx).await?;
        Self::verify_audit_schema(tx).await?;
        Self::verify_identity_audit_schema(tx).await?;
        if Self::scope(tx, scope, false).await? != *workspace
            || Self::identity(tx, scope, username, account_id)
                .await?
                .as_ref()
                != Some(&identity)
            || Self::checked_access_record(tx, scope, identity.binding.principal_id)
                .await?
                .as_ref()
                != Some(&policy)
        {
            return Err(IdentityStoreError::StorageUnavailable);
        }
        Self::require_policy_manager(tx, scope, identity.principal.reference).await?;
        // Fresh bootstrap permits exactly one complete graph, not trigger-injected extra grants.
        for table in [
            "collaboration_authorities",
            "collaboration_workspaces",
            "collaboration_legacy_workspaces",
            "collaboration_principals",
            "collaboration_legacy_identities",
            "collaboration_memberships",
            "collaboration_deployment_grants",
            "collaboration_policy_audit",
            "collaboration_identity_audit",
        ] {
            let count: i64 = sqlx::query(&format!("SELECT count(*) AS count FROM {table}"))
                .fetch_one(&mut **tx)
                .await?
                .try_get("count")?;
            if count != 1 {
                return Err(IdentityStoreError::StorageUnavailable);
            }
        }
        Ok((identity, policy))
    }
}
