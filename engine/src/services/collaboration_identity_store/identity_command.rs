use super::*;
use sha2::{Digest, Sha256};

/// Internal command, not proof of login, ownership of a username, or a verified source account.
/// Only a trusted authenticated adapter may supply the actor and verified account incarnation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyIdentityCommand {
    pub command_id: IdentityCommandId,
    pub actor: PrincipalRef,
    pub workspace: WorkspaceRef,
    pub action: LegacyIdentityAction,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum LegacyIdentityAction {
    Bind {
        username: LegacyUsername,
        account_id: LegacyAccountId,
    },
    Retire {
        username: LegacyUsername,
        account_id: LegacyAccountId,
        expected_principal: PrincipalId,
    },
}

impl LegacyIdentityAction {
    pub(super) fn operation(&self) -> &'static str {
        match self {
            Self::Bind { .. } => "bind",
            Self::Retire { .. } => "retire",
        }
    }
}

impl LegacyIdentityCommand {
    pub(super) fn digest(&self) -> Result<Sha256Digest> {
        let payload = serde_json::to_vec(&(
            1u8,
            self.command_id,
            self.actor,
            self.workspace,
            &self.action,
        ))
        .map_err(|_| IdentityStoreError::InvalidRecord)?;
        Ok(format!("{:x}", Sha256::digest(payload)).parse()?)
    }
}

impl CollaborationIdentityStore {
    pub(super) async fn require_identity_audit(connection: &mut PgConnection) -> Result<()> {
        if Self::version(connection).await? != 2 {
            return Err(IdentityStoreError::IdentityAuditNotEnabled);
        }
        Self::require_policy_audit(connection).await
    }

    /// Attributed staging bind/retirement only. Neither command creates a legacy login account,
    /// changes credentials nor deletes/revokes legacy Sessions. Those need one durable auth
    /// lifecycle adapter before live cutover, not a best-effort call after an unrelated commit.
    pub async fn apply_legacy_identity_command(
        &self,
        command: &LegacyIdentityCommand,
    ) -> Result<LegacyIdentityReceipt> {
        bounded(async {
            let mut tx = self.transaction().await?;
            let receipt = Self::apply_identity_in_transaction(&mut tx, command).await?;
            tx.commit().await?;
            Ok(receipt)
        })
        .await
    }

    /// Crate-internal composition only. Caller owns the transaction and must not publish this
    /// pending receipt until commit. The public trusted-adapter entry point uses the same logic.
    pub(crate) async fn apply_identity_in_transaction(
        tx: &mut Transaction<'_, Postgres>,
        command: &LegacyIdentityCommand,
    ) -> Result<LegacyIdentityReceipt> {
        let digest = command.digest()?;
        let scope = command.workspace;
        let workspace = Self::access_scope(tx, scope, true).await?;
        Self::require_identity_audit(tx).await?;
        // Recheck effective actor even when replaying an already committed receipt.
        Self::require_policy_manager(tx, scope, command.actor).await?;
        let row = sqlx::query(&format!(
            "{} WHERE authority_id = $1 AND command_id = $2 FOR SHARE",
            identity_audit::IDENTITY_AUDIT_SELECT
        ))
        .bind(scope.authority_id.as_uuid())
        .bind(command.command_id.as_uuid())
        .fetch_optional(&mut **tx)
        .await?;
        if let Some(row) = row {
            let entry = LegacyIdentityAuditEntry::decode(&row)?;
            if entry.request_digest.as_ref() != Some(&digest) {
                return Err(IdentityStoreError::CommandConflict);
            }
            return Ok(LegacyIdentityReceipt {
                entry,
                replayed: true,
            });
        }
        let (before, after) = match &command.action {
            LegacyIdentityAction::Bind {
                username,
                account_id,
            } => {
                if workspace.status != WorkspaceStatus::Active {
                    return Err(IdentityStoreError::ScopeInactive);
                }
                Self::bind_identity_record(tx, scope, username, *account_id).await?
            }
            LegacyIdentityAction::Retire {
                username,
                account_id,
                expected_principal,
            } => {
                let identity = Self::identity(tx, scope, username, *account_id)
                    .await?
                    .ok_or(IdentityStoreError::StaleIdentity)?;
                if identity.binding.principal_id != *expected_principal {
                    return Err(IdentityStoreError::StaleIdentity);
                }
                if identity.retired_at.is_none()
                    && identity.principal.status == PrincipalStatus::Active
                    && Self::checked_access_record(tx, scope, *expected_principal)
                        .await?
                        .is_some_and(|value| value.policy.deployment_granted)
                    && !Self::has_policy_manager(tx, scope, Some(*expected_principal)).await?
                {
                    return Err(IdentityStoreError::LastAuthorityManager);
                }
                Self::retire_identity_record(tx, scope, username, *account_id, *expected_principal)
                    .await?
            }
        };
        let entry =
            Self::append_identity_audit(tx, scope, Some(command), before.as_ref(), &after).await?;
        Ok(LegacyIdentityReceipt {
            entry,
            replayed: false,
        })
    }
}
