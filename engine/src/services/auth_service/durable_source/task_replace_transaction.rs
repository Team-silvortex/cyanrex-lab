//! Retained-host Draft replacement; one caller-owned transaction, no publication or commit API.
use super::{
    content_payload::{replacement_content_checks, verify_pinned_contents},
    private_work::{valid_namespace, PrivateWorkContext},
    *,
};
use crate::{
    models::collaboration::{RevisionNumber, TaskContentManifest, TaskId, TaskRef, WorkspaceRef},
    services::{
        artifact_store::ArtifactContentReader,
        task_store::{
            get_content_in_transaction, replace_manual_content_in_transaction, TaskContentSnapshot,
            TaskStoreError,
        },
    },
};

type TaskResult<T> = std::result::Result<T, SessionTaskError>;

pub(crate) struct TaskContentReplaceWorkspace {
    task: String,
    artifacts: String,
    scope: WorkspaceRef,
    reader: ArtifactContentReader,
}

impl TaskContentReplaceWorkspace {
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

    pub(crate) fn validate_manifest(
        scope: WorkspaceRef,
        manifest: &TaskContentManifest,
    ) -> TaskResult<()> {
        if manifest
            .payload()
            .iter()
            .any(|item| item.artifact().workspace != scope)
        {
            return Err(TaskStoreError::ScopeMismatch.into());
        }
        Ok(())
    }

    pub(crate) async fn verify_root(&self) -> TaskResult<()> {
        self.reader.verify_root().await?;
        Ok(())
    }
}

/// Opaque pending snapshot and original authority guard, never a reusable grant or replay token.
pub(crate) struct PreparedTaskContentReplace {
    authority_id: AuthorityId,
    context: PrivateWorkContext,
    pending: TaskContentSnapshot,
}

impl PreparedTaskContentReplace {
    pub(crate) async fn prepare(
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
        workspace: &TaskContentReplaceWorkspace,
        id: TaskId,
        revision: RevisionNumber,
        manifest: TaskContentManifest,
    ) -> TaskResult<Self> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        Self::validate(authority_id, workspace, &manifest)?;
        let mut context = PrivateWorkContext::begin_index_for(
            authority_id,
            tx,
            token,
            &workspace.task,
            workspace.scope,
        )
        .await?;
        let reference = TaskRef {
            workspace: workspace.scope,
            task_id: id,
        };
        // Lock the owned Task first, before any Artifact work. Foreign and absent Tasks agree.
        let old = get_content_in_transaction(tx, workspace.scope, None, reference, context.owner)
            .await?
            .ok_or(TaskStoreError::NotFound)?;
        let checks = replacement_content_checks(&old, revision, &manifest)?;
        context.select_target(tx, &workspace.artifacts).await?;
        verify_pinned_contents(tx, &workspace.reader, context.owner, &checks, false).await?;
        context.select_target(tx, &workspace.task).await?;
        let pending = replace_manual_content_in_transaction(
            tx,
            workspace.scope,
            reference,
            context.owner,
            revision,
            manifest,
        )
        .await?;
        // Flush deferred side effects BEFORE validating both old and new bytes and metadata.
        // Removing an old item must not hide corruption of that item's immutable content.
        sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
            .execute(&mut **tx)
            .await?;
        context.select_target(tx, &workspace.artifacts).await?;
        verify_pinned_contents(tx, &workspace.reader, context.owner, &checks, false).await?;
        context.select_target(tx, &workspace.task).await?;
        if get_content_in_transaction(tx, workspace.scope, None, reference, context.owner)
            .await?
            .as_ref()
            != Some(&pending)
        {
            return Err(TaskStoreError::InvalidRecord.into());
        }
        // Restore the original Source before the host's root and retained-storage postchecks.
        context.finish_retained_for(authority_id, tx).await?;
        Ok(Self {
            authority_id,
            context,
            pending,
        })
    }

    fn validate(
        authority_id: AuthorityId,
        workspace: &TaskContentReplaceWorkspace,
        manifest: &TaskContentManifest,
    ) -> TaskResult<()> {
        if authority_id != workspace.scope.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        TaskContentReplaceWorkspace::validate_manifest(workspace.scope, manifest)
    }

    pub(crate) async fn finish(
        self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> TaskResult<TaskContentSnapshot> {
        self.context
            .finish_from_source_for(self.authority_id, tx)
            .await?;
        Ok(self.pending)
    }
}
