//! Shared borrowed content read and retained-host finalization; no pool, publication or commit.
use super::{
    private_work::{valid_namespace, PrivateWorkContext},
    *,
};
use crate::{
    models::collaboration::{TaskId, TaskRef, WorkspaceRef},
    services::{
        artifact_store::{ArtifactContentReader, ArtifactStoreError},
        task_catalog::TaskDefinitionCatalog,
        task_content::validate_text_payload_content,
        task_store::{get_content_in_transaction, TaskStoreError},
    },
};

type TaskResult<T> = std::result::Result<T, SessionTaskError>;

pub(crate) struct TaskContentReadWorkspace {
    task: String,
    artifacts: String,
    scope: WorkspaceRef,
    reader: ArtifactContentReader,
}

impl TaskContentReadWorkspace {
    pub(crate) fn new(
        task: &str,
        artifacts: &str,
        scope: WorkspaceRef,
        reader: ArtifactContentReader,
    ) -> TaskResult<Self> {
        if !valid_namespace(task) || !valid_namespace(artifacts) || task == artifacts {
            return Err(SessionTaskError::InvalidNamespace);
        }
        if scope != reader.scope() {
            return Err(TaskStoreError::ScopeMismatch.into());
        }
        Ok(Self {
            task: task.into(),
            artifacts: artifacts.into(),
            scope,
            reader,
        })
    }

    pub(crate) async fn verify_root(&self) -> TaskResult<()> {
        self.reader.verify_root().await?;
        Ok(())
    }
}

/// Pending owned content is neither a permission token nor a confirmed read until caller COMMIT.
pub(crate) struct PreparedTaskContentRead {
    authority_id: AuthorityId,
    context: PrivateWorkContext,
    pending: Option<SessionTaskContent>,
}

impl PreparedTaskContentRead {
    pub(crate) async fn prepare(
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
        workspace: &TaskContentReadWorkspace,
        id: TaskId,
    ) -> TaskResult<Self> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        if authority_id != workspace.scope.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        let mut context = PrivateWorkContext::begin_index_for(
            authority_id,
            tx,
            token,
            &workspace.task,
            workspace.scope,
        )
        .await?;
        let pending = read_owned_content_task(
            tx,
            &mut context,
            &workspace.task,
            &workspace.artifacts,
            workspace.scope,
            &workspace.reader,
            id,
            None,
        )
        .await?;
        // Return to the original Source before the host's original-storage/root postchecks.
        context.finish_retained_for(authority_id, tx).await?;
        Ok(Self {
            authority_id,
            context,
            pending,
        })
    }

    pub(crate) async fn finish(
        self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> TaskResult<Option<SessionTaskContent>> {
        self.context
            .finish_from_source_for(self.authority_id, tx)
            .await?;
        Ok(self.pending)
    }
}

/// Both existing Source reads and retained-host reads use this same exact content policy.
#[allow(clippy::too_many_arguments)]
pub(super) async fn read_owned_content_task(
    tx: &mut Transaction<'_, Postgres>,
    context: &mut PrivateWorkContext,
    task_namespace: &str,
    artifact_namespace: &str,
    scope: WorkspaceRef,
    reader: &ArtifactContentReader,
    id: TaskId,
    definitions: Option<&TaskDefinitionCatalog>,
) -> TaskResult<Option<SessionTaskContent>> {
    let owner = context.owner;
    let reference = TaskRef {
        workspace: scope,
        task_id: id,
    };
    // The caller selected the Task namespace. Lock and validate its owned head/event first.
    let Some(snapshot) =
        get_content_in_transaction(tx, scope, definitions, reference, owner).await?
    else {
        return Ok(None);
    };
    context.select_target(tx, artifact_namespace).await?;
    // Even empty content requires the original Artifact namespace and valid schema.
    reader.validate_namespace_in_transaction(tx).await?;
    let mut ordered: Vec<_> = snapshot.manifest.payload().iter().enumerate().collect();
    ordered.sort_by_key(|(_, item)| {
        (
            item.artifact().artifact_id.as_uuid(),
            item.artifact().revision_id.as_uuid(),
        )
    });
    let mut contents = Vec::with_capacity(ordered.len());
    for (index, item) in ordered {
        let content = reader
            .read_in_transaction(tx, item.artifact(), owner)
            .await?
            .ok_or(ArtifactStoreError::NotFound)?;
        validate_text_payload_content(item, owner, &content)
            .map_err(|_| TaskStoreError::InvalidRecord)?;
        contents.push((index, content));
    }
    contents.sort_by_key(|(index, _)| *index);
    context.select_target(tx, task_namespace).await?;
    if get_content_in_transaction(tx, scope, definitions, reference, owner)
        .await?
        .as_ref()
        != Some(&snapshot)
    {
        return Err(TaskStoreError::InvalidRecord.into());
    }
    Ok(Some(SessionTaskContent {
        snapshot,
        contents: contents.into_iter().map(|(_, content)| content).collect(),
    }))
}
