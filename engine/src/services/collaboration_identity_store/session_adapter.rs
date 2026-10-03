//! Narrow crate-internal composition helpers. Never hand these records to an untrusted caller
//! as authentication; the durable source holds one transaction from session check through commit.
use super::*;

impl CollaborationIdentityStore {
    /// Explicit private-work policy: active audited Human membership, not a teaching role or
    /// deployment grant. Caller holds source metadata, registry metadata and authority locks.
    pub(crate) async fn session_private_work_actor(
        tx: &mut Transaction<'_, Postgres>,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
    ) -> Result<PrincipalRef> {
        let workspace = Self::scope(tx, scope, false).await?;
        if workspace.status != WorkspaceStatus::Active {
            return Err(IdentityStoreError::ScopeInactive);
        }
        let identity = Self::identity(tx, scope, username, account_id)
            .await?
            .ok_or(IdentityStoreError::AccessDenied)?;
        if identity.retired_at.is_some()
            || identity.principal.kind != PrincipalKind::Human
            || identity.principal.status != PrincipalStatus::Active
            || !Self::checked_access_record(tx, scope, identity.principal.reference.principal_id)
                .await?
                .is_some_and(|access| access.policy.membership.status == MembershipStatus::Active)
        {
            return Err(IdentityStoreError::AccessDenied);
        }
        Ok(identity.principal.reference)
    }

    pub(crate) async fn session_manager_candidates(
        tx: &mut Transaction<'_, Postgres>,
        scope: WorkspaceRef,
    ) -> Result<Vec<StoredLegacyIdentity>> {
        // Registry validity is necessary, not sufficient: the composition owner must also
        // match a current source account while retaining its source metadata lock.
        let rows = sqlx::query("SELECT g.principal_id FROM collaboration_deployment_grants g
            JOIN collaboration_principals p ON p.authority_id = g.authority_id AND p.principal_id = g.principal_id
            JOIN collaboration_legacy_identities i ON i.authority_id = p.authority_id AND i.principal_id = p.principal_id
            WHERE g.authority_id = $1 AND g.status = 'active' AND p.status = 'active'
            AND p.kind = 'human' AND i.retired_at IS NULL ORDER BY g.principal_id LIMIT 10001")
            .bind(scope.authority_id.as_uuid()).fetch_all(&mut **tx).await?;
        if rows.len() > 10000 {
            return Err(IdentityStoreError::InvalidRecord);
        }
        let mut identities = Vec::with_capacity(rows.len());
        for row in rows {
            let principal_id =
                PrincipalId::try_from(row.try_get::<uuid::Uuid, _>("principal_id")?)?;
            Self::require_policy_manager(
                tx,
                scope,
                PrincipalRef {
                    authority_id: scope.authority_id,
                    principal_id,
                },
            )
            .await?;
            identities.push(Self::session_policy_target(tx, scope, principal_id).await?);
        }
        Ok(identities)
    }

    pub(crate) async fn lock_session_command_scope(
        tx: &mut Transaction<'_, Postgres>,
        scope: WorkspaceRef,
    ) -> Result<()> {
        // The caller already holds auth source metadata. Registry metadata precedes authority
        // and child rows, as for all standalone policy/lifecycle writers.
        Self::access_scope(tx, scope, true).await?;
        Self::require_identity_audit(tx).await
    }

    pub(crate) async fn session_account_manager(
        tx: &mut Transaction<'_, Postgres>,
        scope: WorkspaceRef,
        username: &LegacyUsername,
        account_id: LegacyAccountId,
    ) -> Result<PrincipalRef> {
        let identity = Self::identity(tx, scope, username, account_id)
            .await?
            .ok_or(IdentityStoreError::AccessDenied)?;
        let actor = identity.principal.reference;
        // Checks active Human, exact audited binding and effective explicit deployment grant.
        // Role labels alone never establish management authority.
        Self::require_policy_manager(tx, scope, actor).await?;
        Ok(actor)
    }

    pub(crate) async fn session_policy_target(
        tx: &mut Transaction<'_, Postgres>,
        scope: WorkspaceRef,
        principal: PrincipalId,
    ) -> Result<StoredLegacyIdentity> {
        Self::bound_principal(tx, scope, principal)
            .await?
            .ok_or(IdentityStoreError::StaleIdentity)
    }
}
