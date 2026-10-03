//! Authenticated private content operations. Pending bytes never leave before final Session/commit.
use super::{
    private_work::{valid_namespace, PrivateWorkError},
    *,
};
use crate::{
    models::collaboration::{
        ArtifactContent, ArtifactId, ArtifactRef, ArtifactRevision, ArtifactRevisionId,
        WorkspaceRef,
    },
    services::artifact_store::{ArtifactDraft, ArtifactStore, ArtifactStoreError},
};
use std::path::Path;

/// Trusted server configuration, not a request, owner assertion or separate transaction owner.
#[derive(Clone)]
pub struct SessionArtifactWorkspace {
    pub(super) namespace: String,
    pub(super) scope: WorkspaceRef,
    pub(super) store: ArtifactStore,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionArtifactError {
    #[error(transparent)]
    Session(#[from] SessionCommandError),
    #[error(transparent)]
    Artifact(#[from] ArtifactStoreError),
    #[error("session artifacts require separate pinned non-system namespaces")]
    InvalidNamespace,
}
type ArtifactResult<T> = std::result::Result<T, SessionArtifactError>;
impl From<PrivateWorkError> for SessionArtifactError {
    fn from(error: PrivateWorkError) -> Self {
        match error {
            PrivateWorkError::Session(error) => Self::Session(error),
            PrivateWorkError::InvalidNamespace => Self::InvalidNamespace,
        }
    }
}
impl From<DurableAuthError> for SessionArtifactError {
    fn from(error: DurableAuthError) -> Self {
        Self::Session(error.into())
    }
}
impl From<sqlx::Error> for SessionArtifactError {
    fn from(_: sqlx::Error) -> Self {
        DurableAuthError::StorageUnavailable.into()
    }
}
enum Operation<'a> {
    Create(ArtifactId, ArtifactRevisionId, ArtifactDraft),
    Revise(&'a ArtifactRef, ArtifactRevisionId, ArtifactDraft),
    Read(&'a ArtifactRef),
}
enum Pending {
    Revision(ArtifactRevision),
    Content(Option<ArtifactContent>),
}

impl DurableAuthSource {
    /// Opens an existing private root without creating/chmodding it. No database I/O or installation.
    pub fn open_artifact_workspace(
        &self,
        namespace: &str,
        scope: WorkspaceRef,
        root: &Path,
    ) -> ArtifactResult<SessionArtifactWorkspace> {
        if !valid_namespace(namespace) {
            return Err(SessionArtifactError::InvalidNamespace);
        }
        Ok(SessionArtifactWorkspace {
            namespace: namespace.into(),
            scope,
            store: ArtifactStore::open(self.pool.clone(), scope, root)?,
        })
    }
    pub async fn create_session_artifact(
        &self,
        token: &str,
        workspace: &SessionArtifactWorkspace,
        artifact: ArtifactId,
        revision: ArtifactRevisionId,
        draft: ArtifactDraft,
    ) -> ArtifactResult<ArtifactRevision> {
        match self
            .session_artifact(
                token,
                workspace,
                Operation::Create(artifact, revision, draft),
            )
            .await?
        {
            Pending::Revision(revision) => Ok(revision),
            _ => Err(ArtifactStoreError::InvalidRecord.into()),
        }
    }
    pub async fn revise_session_artifact(
        &self,
        token: &str,
        workspace: &SessionArtifactWorkspace,
        expected: &ArtifactRef,
        revision: ArtifactRevisionId,
        draft: ArtifactDraft,
    ) -> ArtifactResult<ArtifactRevision> {
        match self
            .session_artifact(
                token,
                workspace,
                Operation::Revise(expected, revision, draft),
            )
            .await?
        {
            Pending::Revision(revision) => Ok(revision),
            _ => Err(ArtifactStoreError::InvalidRecord.into()),
        }
    }
    pub async fn read_session_artifact(
        &self,
        token: &str,
        workspace: &SessionArtifactWorkspace,
        reference: &ArtifactRef,
    ) -> ArtifactResult<Option<ArtifactContent>> {
        match self
            .session_artifact(token, workspace, Operation::Read(reference))
            .await?
        {
            Pending::Content(content) => Ok(content),
            _ => Err(ArtifactStoreError::InvalidRecord.into()),
        }
    }
    async fn session_artifact(
        &self,
        token: &str,
        workspace: &SessionArtifactWorkspace,
        operation: Operation<'_>,
    ) -> ArtifactResult<Pending> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.transaction().await?;
            let context = self
                .begin_private_work(&mut tx, token, &workspace.namespace, workspace.scope)
                .await?;
            // Only borrowed-transaction helpers: this store handle never acquires its pool here.
            let pending = match operation {
                Operation::Create(id, revision, draft) => Pending::Revision(
                    workspace
                        .store
                        .create_in_transaction(&mut tx, id, revision, context.owner, draft)
                        .await?,
                ),
                Operation::Revise(expected, revision, draft) => Pending::Revision(
                    workspace
                        .store
                        .revise_in_transaction(&mut tx, expected, revision, context.owner, draft)
                        .await?,
                ),
                Operation::Read(reference) => Pending::Content(
                    workspace
                        .store
                        .read_in_transaction(&mut tx, reference, context.owner)
                        .await?,
                ),
            };
            // A failed check discards pending bytes/metadata. Already-written files may remain.
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(pending)
        })
        .await
        .map_err(|_| SessionArtifactError::from(DurableAuthError::StorageUnavailable))?
    }
}
