use super::*;

/// Trusted operator input for the fixed legacy workspace, never a browser role claim.
/// Deployment authority is explicit and instance-scoped, not inferred from any role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyAccessPolicy {
    pub membership: Membership,
    pub deployment_granted: bool,
}

/// A historical policy record, not a credential or a reusable authorization result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLegacyAccessPolicy {
    pub policy: LegacyAccessPolicy,
    pub revision: RevisionNumber,
}

impl LegacyAccessPolicy {
    fn canonical(&self) -> Result<Self> {
        let mut policy = self.clone();
        let roles = &mut policy.membership.role_refs;
        if roles.len() > 2
            || roles.iter().any(|role| {
                !matches!(
                    role.as_str(),
                    "cyanrex.workspace.owner"
                        | "cyanrex.teaching.teacher"
                        | "cyanrex.teaching.learner"
                )
            })
        {
            return Err(IdentityStoreError::InvalidRecord);
        }
        roles.sort();
        if roles.windows(2).any(|pair| pair[0] == pair[1])
            || (policy.has_role("cyanrex.teaching.teacher")
                && policy.has_role("cyanrex.teaching.learner"))
        {
            return Err(IdentityStoreError::InvalidRecord);
        }
        Ok(policy)
    }

    pub(super) fn has_role(&self, name: &str) -> bool {
        self.membership
            .role_refs
            .iter()
            .any(|role| role.as_str() == name)
    }
}

impl CollaborationIdentityStore {
    pub(super) async fn access_scope(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        writer: bool,
    ) -> Result<Workspace> {
        Self::version(connection).await?;
        Self::access_version(connection).await?;
        // Lock even absent membership keys. Identity retirement and access mutations use the
        // same authority-first protocol; no read of a cached/half-written policy is authorized.
        let statement = if writer {
            "SELECT authority_id FROM collaboration_authorities WHERE authority_id = $1 FOR UPDATE"
        } else {
            "SELECT authority_id FROM collaboration_authorities WHERE authority_id = $1 FOR SHARE"
        };
        sqlx::query(statement)
            .bind(scope.authority_id.as_uuid())
            .fetch_optional(&mut *connection)
            .await?
            .ok_or(IdentityStoreError::ScopeNotFound)?;
        Self::scope(connection, scope, false).await
    }

    pub(super) async fn access_record(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        principal: PrincipalId,
    ) -> Result<Option<StoredLegacyAccessPolicy>> {
        let row = sqlx::query(
            "SELECT role_refs, status, revision FROM collaboration_memberships
            WHERE authority_id = $1 AND workspace_id = $2 AND principal_id = $3 FOR SHARE",
        )
        .bind(scope.authority_id.as_uuid())
        .bind(scope.workspace_id.as_uuid())
        .bind(principal.as_uuid())
        .fetch_optional(&mut *connection)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let revision = row.try_get::<i64, _>("revision")?;
        let grant = sqlx::query(
            "SELECT source_workspace_id, status, revision FROM collaboration_deployment_grants
            WHERE authority_id = $1 AND principal_id = $2 FOR SHARE",
        )
        .bind(scope.authority_id.as_uuid())
        .bind(principal.as_uuid())
        .fetch_optional(connection)
        .await?
        .ok_or(IdentityStoreError::InvalidRecord)?;
        if grant.try_get::<uuid::Uuid, _>("source_workspace_id")? != scope.workspace_id.as_uuid()
            || grant.try_get::<i64, _>("revision")? != revision
        {
            return Err(IdentityStoreError::InvalidRecord);
        }
        let status = match row.try_get::<&str, _>("status")? {
            "active" => MembershipStatus::Active,
            "suspended" => MembershipStatus::Suspended,
            _ => return Err(IdentityStoreError::InvalidRecord),
        };
        let deployment_granted = match grant.try_get::<&str, _>("status")? {
            "active" => true,
            "revoked" => false,
            _ => return Err(IdentityStoreError::InvalidRecord),
        };
        let role_refs = row
            .try_get::<Vec<String>, _>("role_refs")?
            .into_iter()
            .map(|role| role.parse())
            .collect::<std::result::Result<_, _>>()?;
        let policy = LegacyAccessPolicy {
            membership: Membership {
                workspace: scope,
                principal_id: principal,
                role_refs,
                status,
            },
            deployment_granted,
        };
        Ok(Some(StoredLegacyAccessPolicy {
            policy: policy.canonical()?,
            revision: RevisionNumber::try_from(
                u64::try_from(revision).map_err(|_| IdentityStoreError::InvalidRecord)?,
            )?,
        }))
    }

    /// Operator/history read, not authorization. Retirement/disabling blocks effective access;
    /// workspace archival blocks workspace actions, not independent instance deployment grants.
    pub async fn legacy_access(
        &self,
        scope: WorkspaceRef,
        principal: PrincipalId,
    ) -> Result<Option<StoredLegacyAccessPolicy>> {
        bounded(async {
            let mut tx = self.transaction().await?;
            Self::access_scope(&mut tx, scope, false).await?;
            let access = Self::access_record(&mut tx, scope, principal).await?;
            tx.commit().await?;
            Ok(access)
        })
        .await
    }

    /// Explicit trusted maintenance operation, not self-service authorization. `None` means
    /// create only if absent; otherwise the exact reviewed revision must still match, even for
    /// an equal desired policy. Uncertain results require reconciliation, not automatic regrant.
    pub async fn replace_legacy_access(
        &self,
        expected_revision: Option<RevisionNumber>,
        desired: &LegacyAccessPolicy,
    ) -> Result<StoredLegacyAccessPolicy> {
        let desired = desired.canonical()?;
        bounded(async {
            let scope = desired.membership.workspace;
            let principal = desired.membership.principal_id;
            let mut tx = self.transaction().await?;
            let workspace = Self::access_scope(&mut tx, scope, true).await?;
            let identity = Self::bound_principal(&mut tx, scope, principal).await?
                .ok_or(IdentityStoreError::StaleIdentity)?;
            if desired.membership.status == MembershipStatus::Active || desired.deployment_granted {
                if identity.retired_at.is_some() { return Err(IdentityStoreError::IdentityRetired); }
                if identity.principal.status != PrincipalStatus::Active { return Err(IdentityStoreError::IdentityInactive); }
            }
            if desired.membership.status == MembershipStatus::Active && workspace.status != WorkspaceStatus::Active {
                return Err(IdentityStoreError::ScopeInactive);
            }
            let current = Self::access_record(&mut tx, scope, principal).await?;
            if current.as_ref().map(|value| value.revision) != expected_revision {
                return Err(IdentityStoreError::StaleAccess);
            }
            if let Some(current) = &current {
                if current.policy == desired {
                    tx.commit().await?;
                    return Ok(current.clone());
                }
            }
            let revision = RevisionNumber::try_from(current.as_ref().map_or(1, |value| u64::from(value.revision) + 1))?;
            let role_refs: Vec<&str> = desired.membership.role_refs.iter().map(QualifiedName::as_str).collect();
            let status = match desired.membership.status { MembershipStatus::Active => "active", MembershipStatus::Suspended => "suspended" };
            let grant_status = if desired.deployment_granted { "active" } else { "revoked" };
            let rev = u64::from(revision) as i64;
            let previous = expected_revision.map(|value| u64::from(value) as i64);
            let membership_sql = if current.is_some() {
                "UPDATE collaboration_memberships SET role_refs = $4, status = $5, revision = $6, updated_at = NOW()
                WHERE authority_id = $1 AND workspace_id = $2 AND principal_id = $3 AND revision = $7"
            } else {
                "INSERT INTO collaboration_memberships (authority_id, workspace_id, principal_id, role_refs, status, revision)
                SELECT $1, $2, $3, $4, $5, $6 WHERE $7::BIGINT IS NULL"
            };
            confirmed_one(sqlx::query(membership_sql).bind(scope.authority_id.as_uuid()).bind(scope.workspace_id.as_uuid())
                .bind(principal.as_uuid()).bind(role_refs).bind(status).bind(rev).bind(previous)
                .execute(&mut *tx).await?.rows_affected())?;
            let grant_sql = if current.is_some() {
                "UPDATE collaboration_deployment_grants SET status = $4, revision = $5, updated_at = NOW()
                WHERE authority_id = $1 AND source_workspace_id = $2 AND principal_id = $3 AND revision = $6"
            } else {
                "INSERT INTO collaboration_deployment_grants (authority_id, source_workspace_id, principal_id, status, revision)
                SELECT $1, $2, $3, $4, $5 WHERE $6::BIGINT IS NULL"
            };
            confirmed_one(sqlx::query(grant_sql).bind(scope.authority_id.as_uuid()).bind(scope.workspace_id.as_uuid())
                .bind(principal.as_uuid()).bind(grant_status).bind(rev).bind(previous)
                .execute(&mut *tx).await?.rows_affected())?;
            let confirmed = Self::access_record(&mut tx, scope, principal).await?
                .ok_or(IdentityStoreError::StorageUnavailable)?;
            if confirmed.policy != desired || confirmed.revision != revision {
                return Err(IdentityStoreError::StorageUnavailable);
            }
            tx.commit().await?;
            Ok(confirmed)
        }).await
    }
}
