//! Session-authorized staging commands on the SAME connection/transaction as the auth source.
//! No caller-selected actor, cached session, separate pool, public route or automatic activation.
use super::*;
use crate::{
    models::collaboration::{
        IdentityCommandId, PolicyCommandId, PrincipalRef, RevisionNumber, WorkspaceRef,
    },
    services::collaboration_identity_store::{
        CollaborationIdentityStore as Registry, IdentityStoreError, LegacyAccessPolicy,
        LegacyIdentityAction, LegacyIdentityCommand, LegacyIdentityReceipt, LegacyPolicyCommand,
        LegacyPolicyReceipt,
    },
};

/// Requested scope/target only, never credentials or an actor claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBindCommand {
    pub command_id: IdentityCommandId,
    pub workspace: WorkspaceRef,
    pub target: DurableAccountRef,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPolicyCommand {
    pub command_id: PolicyCommandId,
    pub expected_revision: Option<RevisionNumber>,
    pub desired: LegacyAccessPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionCommandError {
    #[error(transparent)]
    Authentication(#[from] DurableAuthError),
    #[error(transparent)]
    Identity(#[from] IdentityStoreError),
    #[error("a current durable session is required")]
    InvalidSession,
    #[error("target does not match the current durable source account")]
    AccountMismatch,
    #[error("self-deletion is not supported by this staging command")]
    SelfDeletionUnsupported,
}
pub(super) type CommandResult<T> = std::result::Result<T, SessionCommandError>;
impl From<sqlx::Error> for SessionCommandError {
    fn from(_: sqlx::Error) -> Self {
        Self::Authentication(DurableAuthError::StorageUnavailable)
    }
}
pub(super) async fn command_bounded<T>(
    operation: impl Future<Output = CommandResult<T>>,
) -> CommandResult<T> {
    tokio::time::timeout(Duration::from_secs(10), operation)
        .await
        .map_err(|_| SessionCommandError::Authentication(DurableAuthError::StorageUnavailable))?
}

impl DurableAuthSource {
    pub(super) async fn command_actor(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        scope: WorkspaceRef,
        digest: &str,
        source_writer: bool,
    ) -> CommandResult<(DurableSession, PrincipalRef)> {
        if scope.authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        // Choose the source lock up front: deletion writes, bind/policy only read. Never upgrade.
        // Auth metadata -> registry metadata -> authority -> account/session/registry child rows.
        self.lock_source(tx, source_writer).await?;
        Registry::lock_session_command_scope(tx, scope).await?;
        let session = self
            .session_record(tx, digest)
            .await?
            .ok_or(SessionCommandError::InvalidSession)?;
        let actor = Registry::session_account_manager(
            tx,
            scope,
            &session.account.username,
            session.account.account_id,
        )
        .await?;
        Ok((session, actor))
    }

    pub(super) async fn command_target(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        target: &DurableAccountRef,
    ) -> CommandResult<()> {
        if target.authority_id != self.authority_id {
            return Err(SessionCommandError::AccountMismatch);
        }
        // Verify identity metadata only; do not pull target credentials into policy operations.
        let row = sqlx::query("SELECT account_id FROM users WHERE username = $1 FOR SHARE")
            .bind(target.username.as_str())
            .fetch_optional(&mut **tx)
            .await?;
        if row
            .map(|row| row.try_get::<Uuid, _>("account_id"))
            .transpose()?
            != Some(target.account_id.as_uuid())
        {
            return Err(SessionCommandError::AccountMismatch);
        }
        Ok(())
    }

    pub(super) async fn recheck_command_session(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        digest: &str,
        expected: &DurableSession,
    ) -> CommandResult<()> {
        // Includes elapsed lock/audit waits and changes caused by triggers in this transaction.
        if self.session_record(tx, digest).await?.as_ref() != Some(expected) {
            return Err(SessionCommandError::InvalidSession);
        }
        Ok(())
    }

    async fn require_source_manager(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        scope: WorkspaceRef,
    ) -> CommandResult<()> {
        for identity in Registry::session_manager_candidates(tx, scope).await? {
            let account = DurableAccountRef {
                authority_id: scope.authority_id,
                username: identity.binding.username,
                account_id: identity.account_id,
            };
            match self.command_target(tx, &account).await {
                Ok(()) => return Ok(()),
                Err(SessionCommandError::AccountMismatch) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(IdentityStoreError::LastAuthorityManager.into())
    }

    /// Bind an existing exact source incarnation using the current session's manager authority.
    /// No registration, implicit grants, retirement or account/session deletion is performed.
    pub async fn bind_session_account(
        &self,
        token: &str,
        request: &SessionBindCommand,
    ) -> CommandResult<LegacyIdentityReceipt> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession);
        }
        command_bounded(async {
            let digest = hash_session_token(token);
            let mut tx = self.transaction().await?;
            let (session, actor) = self
                .command_actor(&mut tx, request.workspace, &digest, false)
                .await?;
            self.command_target(&mut tx, &request.target).await?;
            let command = LegacyIdentityCommand {
                command_id: request.command_id,
                actor,
                workspace: request.workspace,
                action: LegacyIdentityAction::Bind {
                    username: request.target.username.clone(),
                    account_id: request.target.account_id,
                },
            };
            let receipt = Registry::apply_identity_in_transaction(&mut tx, &command).await?;
            self.command_target(&mut tx, &request.target).await?;
            self.recheck_command_session(&mut tx, &digest, &session)
                .await?;
            tx.commit().await?;
            Ok(receipt)
        })
        .await
    }

    /// Session/actor/target checks, policy mutation and audit commit as one database transaction.
    /// Current session and manager authority are required even for a historical receipt replay.
    pub async fn apply_session_policy_command(
        &self,
        token: &str,
        request: &SessionPolicyCommand,
    ) -> CommandResult<LegacyPolicyReceipt> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession);
        }
        command_bounded(async {
            let digest = hash_session_token(token);
            let scope = request.desired.membership.workspace;
            let mut tx = self.transaction().await?;
            let (session, actor) = self.command_actor(&mut tx, scope, &digest, false).await?;
            let identity = Registry::session_policy_target(
                &mut tx,
                scope,
                request.desired.membership.principal_id,
            )
            .await?;
            let target = DurableAccountRef {
                authority_id: scope.authority_id,
                username: identity.binding.username,
                account_id: identity.account_id,
            };
            self.command_target(&mut tx, &target).await?;
            let command = LegacyPolicyCommand {
                command_id: request.command_id,
                actor,
                expected_revision: request.expected_revision,
                desired: request.desired.clone(),
            };
            let receipt = Registry::apply_policy_in_transaction(&mut tx, &command).await?;
            if !receipt.replayed
                && !receipt.entry.after.policy.deployment_granted
                && receipt
                    .entry
                    .before
                    .as_ref()
                    .is_some_and(|before| before.policy.deployment_granted)
            {
                self.require_source_manager(&mut tx, scope).await?;
            }
            self.command_target(&mut tx, &target).await?;
            self.recheck_command_session(&mut tx, &digest, &session)
                .await?;
            tx.commit().await?;
            Ok(receipt)
        })
        .await
    }
}
