//! Exact current subject checks within the original private-work source/target pins.
use super::*;

impl PrivateWorkContext {
    /// A target account is metadata, not authority supplied by a caller. No credentials are read.
    pub async fn require_private_subject(
        &self,
        source: &DurableAuthSource,
        tx: &mut Transaction<'_, Postgres>,
        principal: PrincipalRef,
        account_id: LegacyAccountId,
    ) -> WorkResult<()> {
        if principal.authority_id != self.scope.authority_id {
            return Err(SessionCommandError::AccountMismatch.into());
        }
        self.targets[self.selected].verify(tx).await?;
        NamespacePin::select(tx, &self.source.name).await?;
        self.source.verify(tx).await?;
        self.relations.verify(tx).await?;
        Registry::lock_session_command_scope(tx, self.scope).await?;
        let identity =
            Registry::session_policy_target(tx, self.scope, principal.principal_id).await?;
        if identity.principal.reference != principal || identity.account_id != account_id {
            return Err(SessionCommandError::AccountMismatch.into());
        }
        source
            .command_target(
                tx,
                &DurableAccountRef {
                    authority_id: self.scope.authority_id,
                    username: identity.binding.username.clone(),
                    account_id,
                },
            )
            .await?;
        let current = Registry::session_private_work_actor(
            tx,
            self.scope,
            &identity.binding.username,
            account_id,
        )
        .await?;
        if current != principal {
            return Err(SessionCommandError::AccountMismatch.into());
        }
        self.source.verify(tx).await?;
        self.relations.verify(tx).await?;
        NamespacePin::select(tx, &self.targets[self.selected].name).await?;
        self.targets[self.selected].verify(tx).await?;
        Ok(())
    }
}
