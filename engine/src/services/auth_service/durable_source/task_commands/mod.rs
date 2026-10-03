//! C2-E authenticated private manual tasks on one source-owned transaction. No HTTP/live cutover.
//! Workspace routing is trusted composition configuration, never a client-selected database path.
use super::*;
use crate::{
    models::collaboration::{
        RevisionNumber, TaskId, TaskRef, TaskSnapshot, TaskStatus, WorkspaceRef,
    },
    services::task_store::{TaskDraft, TaskStore, TaskStoreError},
};

use super::private_work::{valid_namespace, PrivateWorkError};

/// Exact server-configured storage namespace, no deserializer or separate pool.
#[derive(Debug, Clone)]
pub struct SessionTaskWorkspace {
    namespace: String,
    scope: WorkspaceRef,
}
impl SessionTaskWorkspace {
    pub fn new(namespace: &str, scope: WorkspaceRef) -> TaskResult<Self> {
        if !valid_namespace(namespace) {
            return Err(SessionTaskError::InvalidNamespace);
        }
        Ok(Self {
            namespace: namespace.into(),
            scope,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionTaskError {
    #[error(transparent)]
    Session(#[from] SessionCommandError),
    #[error(transparent)]
    Task(#[from] TaskStoreError),
    #[error("session tasks require separate pinned non-system namespaces")]
    InvalidNamespace,
    #[error("session tasks support only manual work without artifact references")]
    UnsupportedTask,
}
type TaskResult<T> = std::result::Result<T, SessionTaskError>;
impl From<PrivateWorkError> for SessionTaskError {
    fn from(error: PrivateWorkError) -> Self {
        match error {
            PrivateWorkError::Session(error) => Self::Session(error),
            PrivateWorkError::InvalidNamespace => Self::InvalidNamespace,
        }
    }
}
impl From<DurableAuthError> for SessionTaskError {
    fn from(error: DurableAuthError) -> Self {
        Self::Session(error.into())
    }
}
impl From<sqlx::Error> for SessionTaskError {
    fn from(_: sqlx::Error) -> Self {
        DurableAuthError::StorageUnavailable.into()
    }
}
impl From<crate::services::collaboration_identity_store::IdentityStoreError> for SessionTaskError {
    fn from(error: crate::services::collaboration_identity_store::IdentityStoreError) -> Self {
        Self::Session(error.into())
    }
}
enum Operation {
    Create(TaskId, TaskDraft),
    Read(TaskId),
    Transition(TaskId, RevisionNumber, TaskStatus),
}
fn require_manual(snapshot: &TaskSnapshot) -> TaskResult<()> {
    if snapshot.definition.is_some() || !snapshot.input_refs.is_empty() {
        return Err(SessionTaskError::UnsupportedTask);
    }
    Ok(())
}

impl DurableAuthSource {
    /// No supplied owner, definition, Artifact reference, automatic role assignment or acceptance.
    pub async fn create_session_manual_task(
        &self,
        token: &str,
        workspace: &SessionTaskWorkspace,
        id: TaskId,
        title: &str,
    ) -> TaskResult<TaskSnapshot> {
        let draft = TaskDraft::manual(title, vec![])?;
        self.session_task(token, workspace, Operation::Create(id, draft))
            .await?
            .ok_or_else(|| TaskStoreError::InvalidRecord.into())
    }
    /// Locking authorization read, not a repeatable-read maintenance observation or future grant.
    pub async fn get_session_manual_task(
        &self,
        token: &str,
        workspace: &SessionTaskWorkspace,
        id: TaskId,
    ) -> TaskResult<Option<TaskSnapshot>> {
        self.session_task(token, workspace, Operation::Read(id))
            .await
    }
    pub async fn transition_session_manual_task(
        &self,
        token: &str,
        workspace: &SessionTaskWorkspace,
        id: TaskId,
        revision: RevisionNumber,
        status: TaskStatus,
    ) -> TaskResult<TaskSnapshot> {
        self.session_task(
            token,
            workspace,
            Operation::Transition(id, revision, status),
        )
        .await?
        .ok_or_else(|| TaskStoreError::InvalidRecord.into())
    }
    async fn session_task(
        &self,
        token: &str,
        workspace: &SessionTaskWorkspace,
        operation: Operation,
    ) -> TaskResult<Option<TaskSnapshot>> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.transaction().await?;
            let context = self
                .begin_private_work(&mut tx, token, &workspace.namespace, workspace.scope)
                .await?;
            let owner = context.owner;
            // This handle does not open a second connection: only borrowed-transaction helpers
            // are used below. The source pool owns both namespaces and the single commit.
            let store = TaskStore::new(self.pool.clone(), workspace.scope);
            let reference = |task_id| TaskRef {
                workspace: workspace.scope,
                task_id,
            };
            let pending = match operation {
                Operation::Create(id, draft) => Some(
                    store
                        .create_in_transaction(&mut tx, id, owner, draft)
                        .await?,
                ),
                Operation::Read(id) => {
                    let record = store
                        .get_in_transaction(&mut tx, reference(id), owner)
                        .await?;
                    if let Some(record) = &record {
                        require_manual(record)?;
                    }
                    record
                }
                Operation::Transition(id, revision, status) => {
                    let old = store
                        .get_in_transaction(&mut tx, reference(id), owner)
                        .await?
                        .ok_or(TaskStoreError::NotFound)?;
                    require_manual(&old)?;
                    Some(
                        store
                            .transition_in_transaction(
                                &mut tx,
                                reference(id),
                                owner,
                                revision,
                                status,
                            )
                            .await?,
                    )
                }
            };
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(pending)
        })
        .await
        .map_err(|_| SessionTaskError::from(DurableAuthError::StorageUnavailable))?
    }
}
