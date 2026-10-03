//! Shared private-work policy and namespace guard, never a detached or reusable access grant.
use super::*;
use crate::{
    models::collaboration::{PrincipalRef, WorkspaceRef},
    services::collaboration_identity_store::{
        CollaborationIdentityStore as Registry, IdentityStoreError,
    },
};
mod namespace;
use namespace::NamespacePin;

pub(super) enum PrivateWorkError {
    Session(SessionCommandError),
    InvalidNamespace,
}
type WorkResult<T> = std::result::Result<T, PrivateWorkError>;
impl From<SessionCommandError> for PrivateWorkError {
    fn from(error: SessionCommandError) -> Self {
        Self::Session(error)
    }
}
impl From<DurableAuthError> for PrivateWorkError {
    fn from(error: DurableAuthError) -> Self {
        Self::Session(error.into())
    }
}
impl From<IdentityStoreError> for PrivateWorkError {
    fn from(error: IdentityStoreError) -> Self {
        Self::Session(error.into())
    }
}
impl From<sqlx::Error> for PrivateWorkError {
    fn from(_: sqlx::Error) -> Self {
        DurableAuthError::StorageUnavailable.into()
    }
}
pub(super) fn valid_namespace(name: &str) -> bool {
    namespace::valid_name(name)
}

pub(super) struct PrivateWorkContext {
    pub owner: PrincipalRef,
    scope: WorkspaceRef,
    session: DurableSession,
    digest: String,
    source: NamespacePin,
    target: NamespacePin,
}
impl DurableAuthSource {
    /// Caller owns one source transaction and its timeout, then finishes this guard before commit.
    pub(super) async fn begin_private_work(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
        target: &str,
        scope: WorkspaceRef,
    ) -> WorkResult<PrivateWorkContext> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        let digest = hash_session_token(token);
        let source = NamespacePin::capture(tx).await?;
        if source.name == target {
            return Err(PrivateWorkError::InvalidNamespace);
        }
        namespace::verify_source_relations(tx).await?;
        Self::verify_schema(tx).await?;
        if scope.authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        self.lock_source(tx, false).await?;
        Registry::lock_session_command_scope(tx, scope).await?;
        let session = self
            .session_record(tx, &digest)
            .await?
            .ok_or(SessionCommandError::InvalidSession)?;
        let owner = Registry::session_private_work_actor(
            tx,
            scope,
            &session.account.username,
            session.account.account_id,
        )
        .await?;
        NamespacePin::select(tx, target).await?;
        let selected = NamespacePin::capture(tx).await?;
        if selected.name != target {
            return Err(PrivateWorkError::InvalidNamespace);
        }
        Ok(PrivateWorkContext {
            owner,
            scope,
            session,
            digest,
            source,
            target: selected,
        })
    }
}
impl PrivateWorkContext {
    /// Pending data/bytes must not escape until this succeeds AND the caller confirms commit.
    pub async fn finish(
        self,
        source: &DurableAuthSource,
        tx: &mut Transaction<'_, Postgres>,
    ) -> WorkResult<()> {
        self.target.verify(tx).await?;
        NamespacePin::select(tx, &self.source.name).await?;
        self.source.verify(tx).await?;
        namespace::verify_source_relations(tx).await?;
        source.lock_source(tx, false).await?;
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
        // Fresh database time AFTER all resource/file/registry waits; last check before commit.
        source
            .recheck_command_session(tx, &self.digest, &self.session)
            .await?;
        Ok(())
    }
}
