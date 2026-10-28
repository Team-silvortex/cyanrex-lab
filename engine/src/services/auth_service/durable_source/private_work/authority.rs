//! Pool-free transaction guards. Original pins stay private and never become caller authority.
use super::*;

enum RelationMode {
    Full,
    Index,
}

impl PrivateWorkContext {
    pub(in super::super) async fn begin_for(
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
        target: &str,
        scope: WorkspaceRef,
    ) -> WorkResult<Self> {
        Self::begin(authority_id, tx, token, target, scope, RelationMode::Full).await
    }

    pub(in super::super) async fn begin_index_for(
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
        target: &str,
        scope: WorkspaceRef,
    ) -> WorkResult<Self> {
        Self::begin(authority_id, tx, token, target, scope, RelationMode::Index).await
    }

    async fn begin(
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
        target: &str,
        scope: WorkspaceRef,
        relation_mode: RelationMode,
    ) -> WorkResult<Self> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        let digest = hash_session_token(token);
        let source = NamespacePin::capture(tx).await?;
        if source.name == target {
            return Err(PrivateWorkError::InvalidNamespace);
        }
        let relations = match relation_mode {
            RelationMode::Full => RelationPin::capture(tx, true).await?,
            RelationMode::Index => RelationPin::capture_private_index(tx).await?,
        };
        if scope.authority_id != authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        DurableAuthSource::lock_source_for(authority_id, tx, false).await?;
        DurableAuthSource::verify_schema(tx).await?;
        Registry::lock_session_command_scope(tx, scope).await?;
        let session = session_transaction::record(authority_id, tx, &digest)
            .await?
            .ok_or(SessionCommandError::InvalidSession)?;
        let owner = Registry::session_private_work_actor(
            tx,
            scope,
            &session.account.username,
            session.account.account_id,
        )
        .await?;
        source.verify(tx).await?;
        NamespacePin::select(tx, target).await?;
        let selected = NamespacePin::capture(tx).await?;
        if selected.name != target || selected.has_same_oid(&source) {
            return Err(PrivateWorkError::InvalidNamespace);
        }
        Ok(Self {
            owner,
            scope,
            session,
            digest,
            source,
            relations,
            targets: vec![selected],
            selected: 0,
        })
    }

    /// Original selected target must still be current before returning to the original Source.
    pub(in super::super) async fn finish_retained_for(
        &self,
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
    ) -> WorkResult<()> {
        self.targets[self.selected].verify(tx).await?;
        self.check_authority_for(authority_id, tx).await?;
        self.recheck_session_for(authority_id, tx).await
    }

    /// Sample original current authority again after a target-lock wait, before file dispatch.
    /// Return to that same pinned target; this is not a reusable or continuous permission.
    pub(in super::super) async fn recheck_selected_for(
        &self,
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
    ) -> WorkResult<()> {
        let selected = &self.targets[self.selected];
        selected.verify(tx).await?;
        self.check_authority_for(authority_id, tx).await?;
        self.recheck_session_for(authority_id, tx).await?;
        NamespacePin::select(tx, &selected.name).await?;
        selected.verify(tx).await?;
        Ok(())
    }

    /// A host guard runs in Source after preparation. Refuse namespace drift before selecting
    /// anything, then recheck every original pin and membership before the final fresh Session.
    pub(in super::super) async fn finish_from_source_for(
        self,
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
    ) -> WorkResult<()> {
        self.source.verify(tx).await?;
        self.relations.verify(tx).await?;
        self.check_authority_for(authority_id, tx).await?;
        self.recheck_session_for(authority_id, tx).await
    }

    async fn check_authority_for(
        &self,
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
    ) -> WorkResult<()> {
        for target in &self.targets {
            NamespacePin::select(tx, &target.name).await?;
            target.verify(tx).await?;
        }
        NamespacePin::select(tx, &self.source.name).await?;
        self.source.verify(tx).await?;
        self.relations.verify(tx).await?;
        DurableAuthSource::lock_source_for(authority_id, tx, false).await?;
        Registry::lock_session_command_scope(tx, self.scope).await?;
        let current = Registry::session_private_work_actor(
            tx,
            self.scope,
            &self.session.account.username,
            self.session.account.account_id,
        )
        .await?;
        if current != self.owner {
            return Err(SessionCommandError::InvalidSession.into());
        }
        Ok(())
    }

    async fn recheck_session_for(
        &self,
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
    ) -> WorkResult<()> {
        if session_transaction::record(authority_id, tx, &self.digest)
            .await?
            .as_ref()
            != Some(&self.session)
        {
            return Err(SessionCommandError::InvalidSession.into());
        }
        Ok(())
    }
}
