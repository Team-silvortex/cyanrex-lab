//! Private Task inputs: one source transaction, pinned namespaces and exact owned bytes.
//! No Review authority, implicit sharing, public route or file mutation.
use super::*;
use crate::{
    models::collaboration::{ArtifactContent, ArtifactRef, PrincipalRef},
    services::artifact_store::ArtifactStoreError,
};

mod catalog;
mod replacement;
pub use catalog::SessionCatalogTaskWorkspace;

/// Trusted server composition of two initialized stores; no request deserializer or new pool.
#[derive(Clone)]
pub struct SessionTaskInputsWorkspace {
    tasks: SessionTaskWorkspace,
    artifacts: SessionArtifactWorkspace,
}
impl SessionTaskInputsWorkspace {
    pub fn new(
        tasks: SessionTaskWorkspace,
        artifacts: SessionArtifactWorkspace,
    ) -> TaskResult<Self> {
        if tasks.namespace == artifacts.namespace {
            return Err(SessionTaskError::InvalidNamespace);
        }
        if tasks.scope != artifacts.scope {
            return Err(TaskStoreError::ScopeMismatch.into());
        }
        Ok(Self { tasks, artifacts })
    }

    /// Stable lock order, but retain original input order. Mutations discard one verified blob
    /// at a time; only a read accumulates the bounded (32 * 1 MiB) result until final commit.
    async fn verify_inputs(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        owner: PrincipalRef,
        references: &[ArtifactRef],
        retain: bool,
    ) -> TaskResult<Vec<ArtifactContent>> {
        // Empty catalog inputs still require the configured Artifact namespace and metadata.
        self.artifacts
            .store
            .validate_namespace_in_transaction(tx)
            .await?;
        let mut ordered: Vec<_> = references.iter().enumerate().collect();
        ordered.sort_by_key(|(_, reference)| {
            (
                reference.artifact_id.as_uuid(),
                reference.revision_id.as_uuid(),
            )
        });
        let mut contents = Vec::new();
        for (index, reference) in ordered {
            let content = self
                .artifacts
                .store
                .read_in_transaction(tx, reference, owner)
                .await?
                .ok_or(ArtifactStoreError::NotFound)?;
            if retain {
                contents.push((index, content));
            }
        }
        contents.sort_by_key(|(index, _)| *index);
        Ok(contents.into_iter().map(|(_, content)| content).collect())
    }
}

/// A committed, exact read result, not a grant for a later operation or verified Review evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTaskInputs {
    pub task: TaskSnapshot,
    pub inputs: Vec<ArtifactContent>,
}

enum InputOperation {
    Create(TaskDraft, Vec<ArtifactRef>),
    Read,
    Transition(RevisionNumber, TaskStatus),
    ReplaceInputs(RevisionNumber, Vec<ArtifactRef>),
}
enum InputPending {
    Task(TaskSnapshot),
    Read(Option<SessionTaskInputs>),
}

enum InputPolicy<'a> {
    Manual,
    Catalog(&'a SessionCatalogTaskWorkspace),
}
impl InputPolicy<'_> {
    fn require_task(&self, task: &TaskSnapshot) -> TaskResult<()> {
        match self {
            Self::Manual => {
                if task.definition.is_some()
                    || task.input_refs.is_empty()
                    || task.input_refs.len() > 32
                {
                    return Err(SessionTaskError::UnsupportedTask);
                }
                Ok(())
            }
            Self::Catalog(workspace) => workspace.require_task(task),
        }
    }
}

impl DurableAuthSource {
    /// Only this member's exact immutable inputs; no caller-selected owner or domain definition.
    pub async fn create_session_input_task(
        &self,
        token: &str,
        workspace: &SessionTaskInputsWorkspace,
        id: TaskId,
        title: &str,
        inputs: Vec<ArtifactRef>,
    ) -> TaskResult<TaskSnapshot> {
        if inputs.is_empty() {
            return Err(SessionTaskError::UnsupportedTask);
        }
        // Bound the duplicate needed for in-transaction checking before cloning caller input.
        if inputs.len() > 32 {
            return Err(TaskStoreError::InvalidInput.into());
        }
        let draft = TaskDraft::manual(title, inputs.clone())?;
        match self
            .session_input_task(
                token,
                workspace,
                id,
                InputOperation::Create(draft, inputs),
                InputPolicy::Manual,
            )
            .await?
        {
            InputPending::Task(task) => Ok(task),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    /// Read the locked Task and its saved inputs together. Unknown/other-owned Tasks return none.
    pub async fn get_session_input_task(
        &self,
        token: &str,
        workspace: &SessionTaskInputsWorkspace,
        id: TaskId,
    ) -> TaskResult<Option<SessionTaskInputs>> {
        match self
            .session_input_task(
                token,
                workspace,
                id,
                InputOperation::Read,
                InputPolicy::Manual,
            )
            .await?
        {
            InputPending::Read(result) => Ok(result),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    /// Every transition, including cancellation, requires the saved content to remain valid.
    pub async fn transition_session_input_task(
        &self,
        token: &str,
        workspace: &SessionTaskInputsWorkspace,
        id: TaskId,
        revision: RevisionNumber,
        status: TaskStatus,
    ) -> TaskResult<TaskSnapshot> {
        match self
            .session_input_task(
                token,
                workspace,
                id,
                InputOperation::Transition(revision, status),
                InputPolicy::Manual,
            )
            .await?
        {
            InputPending::Task(task) => Ok(task),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    async fn session_input_task(
        &self,
        token: &str,
        workspace: &SessionTaskInputsWorkspace,
        id: TaskId,
        operation: InputOperation,
        policy: InputPolicy<'_>,
    ) -> TaskResult<InputPending> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.transaction().await?;
            let mut context = self
                .begin_private_work(
                    &mut tx,
                    token,
                    &workspace.tasks.namespace,
                    workspace.tasks.scope,
                )
                .await?;
            let owner = context.owner;
            let reference = TaskRef {
                workspace: workspace.tasks.scope,
                task_id: id,
            };
            let store = TaskStore::new(self.pool.clone(), workspace.tasks.scope);
            // Lock Task metadata and any owned row BEFORE Artifact metadata/heads. Standalone
            // trusted writers must not replace the Task snapshot while its references are checked.
            let old = store.get_in_transaction(&mut tx, reference, owner).await?;
            let references = match &operation {
                InputOperation::Create(_, inputs) => {
                    if old.is_some() {
                        return Err(TaskStoreError::Conflict.into());
                    }
                    inputs.clone()
                }
                InputOperation::Read if old.is_none() => {
                    context.finish(self, &mut tx).await?;
                    tx.commit().await?;
                    return Ok(InputPending::Read(None));
                }
                _ => {
                    let old = old.as_ref().ok_or(TaskStoreError::NotFound)?;
                    policy.require_task(old)?;
                    if let InputOperation::Transition(revision, status) = &operation {
                        if old.revision != *revision {
                            return Err(TaskStoreError::StaleRevision.into());
                        }
                        if !old.status.can_transition_to(*status) {
                            return Err(TaskStoreError::InvalidTransition.into());
                        }
                    }
                    if let InputOperation::ReplaceInputs(revision, inputs) = &operation {
                        replacement::replacement_references(old, *revision, inputs, &policy)?
                    } else {
                        old.input_refs.clone()
                    }
                }
            };
            // Pin the third namespace BEFORE any Task write. Revisits must verify the original
            // name/OID, not accept an identical clone substituted by a Task/outbox trigger.
            context
                .select_target(&mut tx, &workspace.artifacts.namespace)
                .await?;
            let read = matches!(&operation, InputOperation::Read);
            let inputs = workspace
                .verify_inputs(&mut tx, owner, &references, read)
                .await?;
            context
                .select_target(&mut tx, &workspace.tasks.namespace)
                .await?;
            let task = match operation {
                InputOperation::Create(draft, _) => {
                    store
                        .create_in_transaction(&mut tx, id, owner, draft)
                        .await?
                }
                InputOperation::Read => old.ok_or(TaskStoreError::NotFound)?,
                InputOperation::Transition(revision, status) => {
                    store
                        .transition_in_transaction(&mut tx, reference, owner, revision, status)
                        .await?
                }
                InputOperation::ReplaceInputs(revision, inputs) => {
                    store
                        .replace_inputs_in_transaction(&mut tx, reference, owner, revision, inputs)
                        .await?
                }
            };
            policy.require_task(&task)?;
            if !read {
                // Row locks exclude other ordinary writers, not a trigger's own-transaction
                // side effects. Validate all referenced facts again after the Task/event writes.
                context
                    .select_target(&mut tx, &workspace.artifacts.namespace)
                    .await?;
                workspace
                    .verify_inputs(&mut tx, owner, &references, false)
                    .await?;
                context
                    .select_target(&mut tx, &workspace.tasks.namespace)
                    .await?;
            }
            if store
                .get_in_transaction(&mut tx, reference, owner)
                .await?
                .as_ref()
                != Some(&task)
            {
                return Err(TaskStoreError::InvalidRecord.into());
            }
            // No Task result or retained content escapes before every pin, membership and the
            // exact Session has been rechecked, then the one source-owned commit is confirmed.
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(if read {
                InputPending::Read(Some(SessionTaskInputs { task, inputs }))
            } else {
                InputPending::Task(task)
            })
        })
        .await
        .map_err(|_| SessionTaskError::from(DurableAuthError::StorageUnavailable))?
    }
}
