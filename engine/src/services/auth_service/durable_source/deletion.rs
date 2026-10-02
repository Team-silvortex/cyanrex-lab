//! Restricted administrative deletion, without source installation, bootstrap or live cutover.
use super::{
    session_commands::{command_bounded, CommandResult},
    *,
};
use crate::{
    models::collaboration::{IdentityCommandId, PrincipalId, PrincipalStatus, WorkspaceRef},
    services::collaboration_identity_store::{
        CollaborationIdentityStore as Registry, IdentityStoreError, LegacyIdentityAction,
        LegacyIdentityCommand, LegacyIdentityReceipt,
    },
};

/// Exact reviewed account and binding, never a caller-selected actor or username-only delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDeleteAccountCommand {
    pub command_id: IdentityCommandId,
    pub workspace: WorkspaceRef,
    pub target: DurableAccountRef,
    pub expected_principal: PrincipalId,
}

impl DurableAuthSource {
    /// Delete another active, bound account and all its Sessions with audited retirement.
    /// Returns only after commit acknowledgement; the identity receipt alone is NOT a durable
    /// deletion receipt. No successful replay, self-deletion, unbound or already-retired cleanup.
    /// An uncertain commit requires reconciliation of source AND registry; never change IDs
    /// to compensate, infer rollback from an error, or treat historical Retire as delete proof.
    pub async fn delete_session_account(
        &self,
        token: &str,
        request: &SessionDeleteAccountCommand,
    ) -> CommandResult<LegacyIdentityReceipt> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession);
        }
        command_bounded(async {
            let digest = hash_session_token(token);
            let scope = request.workspace;
            let mut tx = self.transaction().await?;
            // Source FOR UPDATE precedes registry metadata/authority and all child rows.
            // Starting with SHARE and upgrading after actor resolution can deadlock deletions.
            let (session, actor) = self.command_actor(&mut tx, scope, &digest, true).await?;
            self.command_target(&mut tx, &request.target).await?;
            if session.account == request.target {
                return Err(SessionCommandError::SelfDeletionUnsupported);
            }
            let identity = Registry::session_policy_target(&mut tx, scope, request.expected_principal).await?;
            if identity.binding.username != request.target.username || identity.account_id != request.target.account_id {
                return Err(IdentityStoreError::StaleIdentity.into());
            }
            if identity.retired_at.is_some() {
                return Err(IdentityStoreError::IdentityRetired.into());
            }
            if identity.principal.status != PrincipalStatus::Active {
                return Err(IdentityStoreError::IdentityInactive.into());
            }
            let command = LegacyIdentityCommand {
                command_id: request.command_id, actor, workspace: scope,
                action: LegacyIdentityAction::Retire {
                    username: request.target.username.clone(), account_id: request.target.account_id,
                    expected_principal: request.expected_principal,
                },
            };
            let receipt = Registry::apply_identity_in_transaction(&mut tx, &command).await?;
            // A standalone retirement receipt must never authorize a new source mutation.
            if receipt.replayed {
                return Err(IdentityStoreError::CommandConflict.into());
            }
            self.command_target(&mut tx, &request.target).await?;
            let sessions: i64 = sqlx::query("SELECT count(*) AS count FROM sessions WHERE username = $1 AND account_id = $2")
                .bind(request.target.username.as_str()).bind(request.target.account_id.as_uuid())
                .fetch_one(&mut *tx).await?.try_get("count")?;
            let removed = sqlx::query("DELETE FROM sessions WHERE username = $1 AND account_id = $2")
                .bind(request.target.username.as_str()).bind(request.target.account_id.as_uuid())
                .execute(&mut *tx).await?.rows_affected();
            if removed != sessions as u64 {
                return Err(DurableAuthError::StorageUnavailable.into());
            }
            confirmed_one(sqlx::query("DELETE FROM users WHERE username = $1 AND account_id = $2")
                .bind(request.target.username.as_str()).bind(request.target.account_id.as_uuid())
                .execute(&mut *tx).await?.rows_affected())?;
            // Check final absence, including reinserts by triggers; never delete a replacement.
            let remains: bool = sqlx::query("SELECT EXISTS (SELECT 1 FROM users WHERE username = $1 OR account_id = $2)
                OR EXISTS (SELECT 1 FROM sessions WHERE username = $1 OR account_id = $2) AS remains")
                .bind(request.target.username.as_str()).bind(request.target.account_id.as_uuid())
                .fetch_one(&mut *tx).await?.try_get("remains")?;
            if remains || Registry::session_policy_target(&mut tx, scope, request.expected_principal).await? != receipt.entry.after {
                return Err(DurableAuthError::StorageUnavailable.into());
            }
            // The surviving actor is an audited, source-backed manager, not a phantom grant.
            // Recheck after source writes too, since triggers may change identity/policy/session.
            if Registry::session_account_manager(&mut tx, scope, &session.account.username, session.account.account_id).await? != actor {
                return Err(IdentityStoreError::AccessDenied.into());
            }
            self.recheck_command_session(&mut tx, &digest, &session).await?;
            tx.commit().await?;
            Ok(receipt)
        }).await
    }
}
