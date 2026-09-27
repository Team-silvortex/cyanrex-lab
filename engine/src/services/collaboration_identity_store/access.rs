use super::*;

/// Trusted operator input for the fixed legacy workspace, never a browser role claim.
/// Deployment authority is explicit and instance-scoped, not inferred from any role.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyAccessPolicy {
    pub membership: Membership,
    pub deployment_granted: bool,
}

/// A historical policy record, not a credential or a reusable authorization result.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredLegacyAccessPolicy {
    pub policy: LegacyAccessPolicy,
    pub revision: RevisionNumber,
}

impl LegacyAccessPolicy {
    pub(super) fn canonical(&self) -> Result<Self> {
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
            let access = Self::checked_access_record(&mut tx, scope, principal).await?;
            tx.commit().await?;
            Ok(access)
        })
        .await
    }

    /// Pre-audit trusted maintenance only; schema v2 requires an attributed command. `None` means
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
            let mut tx = self.transaction().await?;
            let workspace = Self::access_scope(&mut tx, scope, true).await?;
            if Self::access_version(&mut tx).await? != 1 {
                return Err(IdentityStoreError::AuditContextRequired);
            }
            let (_, confirmed) =
                Self::replace_access_record(&mut tx, &workspace, expected_revision, &desired)
                    .await?;
            tx.commit().await?;
            Ok(confirmed)
        })
        .await
    }
}
