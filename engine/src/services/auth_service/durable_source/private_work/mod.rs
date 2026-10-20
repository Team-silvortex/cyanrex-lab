//! Shared private-work policy and namespace guard, never a detached or reusable access grant.
use super::*;
use super::{
    namespace::{self, NamespaceError, NamespacePin},
    source_relations::RelationPin,
};
use crate::{
    models::collaboration::{PrincipalRef, WorkspaceRef},
    services::collaboration_identity_store::{
        CollaborationIdentityStore as Registry, IdentityStoreError,
    },
};
mod authority;
mod subjects;
#[cfg(unix)]
mod timed;

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
impl From<NamespaceError> for PrivateWorkError {
    fn from(error: NamespaceError) -> Self {
        match error {
            NamespaceError::InvalidNamespace => Self::InvalidNamespace,
            NamespaceError::StorageUnavailable => DurableAuthError::StorageUnavailable.into(),
        }
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
    relations: RelationPin,
    targets: Vec<NamespacePin>,
    selected: usize,
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
        PrivateWorkContext::begin_for(self.authority_id, tx, token, target, scope).await
    }
}
impl PrivateWorkContext {
    pub(super) fn account_id(&self) -> LegacyAccountId {
        self.session.account.account_id
    }

    pub(super) fn source_namespace(&self) -> &str {
        &self.source.name
    }

    /// Keep every first-observed namespace identity; revisiting cannot silently replace its pin.
    pub async fn select_target(
        &mut self,
        tx: &mut Transaction<'_, Postgres>,
        name: &str,
    ) -> WorkResult<()> {
        self.targets[self.selected].verify(tx).await?;
        if name == self.source.name {
            return Err(PrivateWorkError::InvalidNamespace);
        }
        NamespacePin::select(tx, name).await?;
        if let Some(index) = self.targets.iter().position(|target| target.name == name) {
            self.targets[index].verify(tx).await?;
            self.selected = index;
        } else {
            let selected = NamespacePin::capture(tx).await?;
            if selected.name != name || selected.has_same_oid(&self.source) {
                return Err(PrivateWorkError::InvalidNamespace);
            }
            self.targets.push(selected);
            self.selected = self.targets.len() - 1;
        }
        Ok(())
    }

    /// Pending data/bytes must not escape until this succeeds AND the caller confirms commit.
    pub async fn finish(
        self,
        source: &DurableAuthSource,
        tx: &mut Transaction<'_, Postgres>,
    ) -> WorkResult<()> {
        self.finish_retained(source, tx).await
    }

    /// Same checks as finish, but retains the opaque original pins for a separate file-work gap.
    /// This observation grants no later command authority; resume and finalization must recheck it.
    pub(super) async fn finish_retained(
        &self,
        source: &DurableAuthSource,
        tx: &mut Transaction<'_, Postgres>,
    ) -> WorkResult<()> {
        self.finish_retained_for(source.authority_id, tx).await
    }
}
