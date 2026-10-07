use super::*;
use sha2::{Digest, Sha256};

/// Internal trusted-adapter input, not a login credential or a public HTTP request body.
/// The adapter must resolve actor from its verified session; never accept a browser's actor ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyPolicyCommand {
    pub command_id: PolicyCommandId,
    pub actor: PrincipalRef,
    pub expected_revision: Option<RevisionNumber>,
    pub desired: LegacyAccessPolicy,
}

impl LegacyPolicyCommand {
    pub(super) fn digest(&self) -> Result<Sha256Digest> {
        let payload = serde_json::to_vec(&(
            1u8,
            self.command_id,
            self.actor,
            self.expected_revision,
            self.desired.canonical()?,
        ))
        .map_err(|_| IdentityStoreError::InvalidRecord)?;
        Ok(format!("{:x}", Sha256::digest(payload)).parse()?)
    }
}

impl CollaborationIdentityStore {
    pub(super) async fn require_policy_audit(connection: &mut PgConnection) -> Result<()> {
        if Self::access_version(connection).await? != 2 {
            return Err(IdentityStoreError::AuditNotEnabled);
        }
        Ok(())
    }

    pub(super) async fn require_policy_manager(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        actor: PrincipalRef,
    ) -> Result<()> {
        if actor.authority_id != scope.authority_id
            || !Self::active_legacy_principal(connection, scope, actor.principal_id).await?
            || !Self::checked_access_record(connection, scope, actor.principal_id)
                .await?
                .is_some_and(|value| value.policy.deployment_granted)
        {
            return Err(IdentityStoreError::AccessDenied);
        }
        Ok(())
    }

    pub(super) async fn has_policy_manager(
        connection: &mut PgConnection,
        scope: WorkspaceRef,
        excluding: Option<PrincipalId>,
    ) -> Result<bool> {
        // Explicit instance grants are independent of workspace/member status and teacher role.
        let rows = sqlx::query("SELECT g.principal_id FROM collaboration_deployment_grants g
            JOIN collaboration_principals p ON p.authority_id = g.authority_id AND p.principal_id = g.principal_id
            JOIN collaboration_legacy_identities i ON i.authority_id = p.authority_id AND i.principal_id = p.principal_id
            WHERE g.authority_id = $1 AND g.status = 'active' AND p.status = 'active'
            AND p.kind = 'human' AND i.retired_at IS NULL AND ($2::UUID IS NULL OR g.principal_id <> $2)
            ORDER BY g.principal_id LIMIT 10001")
            .bind(scope.authority_id.as_uuid()).bind(excluding.map(PrincipalId::as_uuid))
            .fetch_all(&mut *connection).await?;
        if rows.len() > 10000 {
            return Err(IdentityStoreError::InvalidRecord);
        }
        for row in rows {
            let principal = PrincipalId::try_from(row.try_get::<uuid::Uuid, _>("principal_id")?)?;
            if Self::active_legacy_principal(connection, scope, principal).await?
                && Self::checked_access_record(connection, scope, principal)
                    .await?
                    .is_some_and(|value| value.policy.deployment_granted)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Atomically authorize, compare revision, replace both policy rows, and append receipt.
    /// Retrying an uncertain result must reuse the exact command ID and payload. No implicit
    /// retry, new ID, regrant, or success on storage errors. Current actor authority is required
    /// even for replay; a historical receipt never restores an old effective policy.
    pub async fn apply_legacy_policy_command(
        &self,
        command: &LegacyPolicyCommand,
    ) -> Result<LegacyPolicyReceipt> {
        // Preserve validation-before-storage behavior of the existing trusted-adapter API.
        let command = LegacyPolicyCommand {
            desired: command.desired.canonical()?,
            ..command.clone()
        };
        bounded(async {
            let mut tx = self.transaction().await?;
            let receipt = Self::apply_policy_in_transaction(&mut tx, &command).await?;
            tx.commit().await?;
            Ok(receipt)
        })
        .await
    }

    /// Pending receipt only; the composition owner must retain all admission locks and commit
    /// before publishing. A Transaction argument prevents accidental autocommit composition.
    pub(crate) async fn apply_policy_in_transaction(
        tx: &mut Transaction<'_, Postgres>,
        command: &LegacyPolicyCommand,
    ) -> Result<LegacyPolicyReceipt> {
        let command = LegacyPolicyCommand {
            desired: command.desired.canonical()?,
            ..command.clone()
        };
        let digest = command.digest()?;
        let scope = command.desired.membership.workspace;
        let principal = command.desired.membership.principal_id;
        let workspace = Self::access_scope(tx, scope, true).await?;
        Self::require_policy_audit(tx).await?;
        Self::require_policy_manager(tx, scope, command.actor).await?;
        let row = sqlx::query(&format!(
            "{} WHERE authority_id = $1 AND command_id = $2 FOR SHARE",
            policy_audit::audit_select(3)
        ))
        .bind(scope.authority_id.as_uuid())
        .bind(command.command_id.as_uuid())
        .bind(DateTime::<Utc>::MAX_UTC)
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(row) = row {
            let entry = LegacyPolicyAuditEntry::decode(&row)?;
            if entry.request_digest.as_ref() != Some(&digest) {
                return Err(IdentityStoreError::CommandConflict);
            }
            return Ok(LegacyPolicyReceipt {
                entry,
                replayed: true,
            });
        }
        let current = Self::checked_access_record(tx, scope, principal).await?;
        if current.as_ref().map(|value| value.revision) != command.expected_revision {
            return Err(IdentityStoreError::StaleAccess);
        }
        if !command.desired.deployment_granted
            && current
                .as_ref()
                .is_some_and(|value| value.policy.deployment_granted)
            && !Self::has_policy_manager(tx, scope, Some(principal)).await?
        {
            return Err(IdentityStoreError::LastAuthorityManager);
        }
        let (before, after) = Self::replace_access_record(
            tx,
            &workspace,
            command.expected_revision,
            &command.desired,
        )
        .await?;
        let entry = Self::append_policy_audit(tx, Some(&command), before.as_ref(), &after).await?;
        Ok(LegacyPolicyReceipt {
            entry,
            replayed: false,
        })
    }
}
