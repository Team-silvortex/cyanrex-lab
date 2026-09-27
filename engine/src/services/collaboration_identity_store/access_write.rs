use super::*;

impl CollaborationIdentityStore {
    /// Caller holds the authority writer lock and owns commit, including any required audit.
    pub(super) async fn replace_access_record(
        connection: &mut PgConnection,
        workspace: &Workspace,
        expected_revision: Option<RevisionNumber>,
        desired: &LegacyAccessPolicy,
    ) -> Result<(Option<StoredLegacyAccessPolicy>, StoredLegacyAccessPolicy)> {
        let scope = desired.membership.workspace;
        let principal = desired.membership.principal_id;
        let identity = Self::bound_principal(connection, scope, principal)
            .await?
            .ok_or(IdentityStoreError::StaleIdentity)?;
        if desired.membership.status == MembershipStatus::Active || desired.deployment_granted {
            if identity.retired_at.is_some() {
                return Err(IdentityStoreError::IdentityRetired);
            }
            if identity.principal.status != PrincipalStatus::Active {
                return Err(IdentityStoreError::IdentityInactive);
            }
        }
        if desired.membership.status == MembershipStatus::Active
            && workspace.status != WorkspaceStatus::Active
        {
            return Err(IdentityStoreError::ScopeInactive);
        }
        let current = Self::checked_access_record(connection, scope, principal).await?;
        if current.as_ref().map(|value| value.revision) != expected_revision {
            return Err(IdentityStoreError::StaleAccess);
        }
        if let Some(current) = &current {
            if current.policy == *desired {
                return Ok((Some(current.clone()), current.clone()));
            }
        }
        let revision = RevisionNumber::try_from(
            current
                .as_ref()
                .map_or(1, |value| u64::from(value.revision) + 1),
        )?;
        let role_refs: Vec<&str> = desired
            .membership
            .role_refs
            .iter()
            .map(QualifiedName::as_str)
            .collect();
        let status = match desired.membership.status {
            MembershipStatus::Active => "active",
            MembershipStatus::Suspended => "suspended",
        };
        let grant_status = if desired.deployment_granted {
            "active"
        } else {
            "revoked"
        };
        let rev = u64::from(revision) as i64;
        let previous = expected_revision.map(|value| u64::from(value) as i64);
        let membership_sql = if current.is_some() {
            "UPDATE collaboration_memberships SET role_refs = $4, status = $5, revision = $6, updated_at = NOW()
            WHERE authority_id = $1 AND workspace_id = $2 AND principal_id = $3 AND revision = $7"
        } else {
            "INSERT INTO collaboration_memberships (authority_id, workspace_id, principal_id, role_refs, status, revision)
            SELECT $1, $2, $3, $4, $5, $6 WHERE $7::BIGINT IS NULL"
        };
        confirmed_one(
            sqlx::query(membership_sql)
                .bind(scope.authority_id.as_uuid())
                .bind(scope.workspace_id.as_uuid())
                .bind(principal.as_uuid())
                .bind(role_refs)
                .bind(status)
                .bind(rev)
                .bind(previous)
                .execute(&mut *connection)
                .await?
                .rows_affected(),
        )?;
        let grant_sql = if current.is_some() {
            "UPDATE collaboration_deployment_grants SET status = $4, revision = $5, updated_at = NOW()
            WHERE authority_id = $1 AND source_workspace_id = $2 AND principal_id = $3 AND revision = $6"
        } else {
            "INSERT INTO collaboration_deployment_grants (authority_id, source_workspace_id, principal_id, status, revision)
            SELECT $1, $2, $3, $4, $5 WHERE $6::BIGINT IS NULL"
        };
        confirmed_one(
            sqlx::query(grant_sql)
                .bind(scope.authority_id.as_uuid())
                .bind(scope.workspace_id.as_uuid())
                .bind(principal.as_uuid())
                .bind(grant_status)
                .bind(rev)
                .bind(previous)
                .execute(&mut *connection)
                .await?
                .rows_affected(),
        )?;
        // The audit head is intentionally still the old policy until our caller appends receipt.
        let confirmed = Self::access_record(connection, scope, principal)
            .await?
            .ok_or(IdentityStoreError::StorageUnavailable)?;
        if confirmed.policy != *desired || confirmed.revision != revision {
            return Err(IdentityStoreError::StorageUnavailable);
        }
        Ok((current, confirmed))
    }
}
