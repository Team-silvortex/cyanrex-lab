//! Empty manual Task creation on a caller-owned transaction, never a confirmed save by itself.
use super::{
    private_work::{valid_namespace, PrivateWorkContext},
    *,
};
use crate::{
    models::collaboration::{TaskContentManifest, TaskId, TaskRef, WorkspaceRef},
    services::{
        artifact_store::validate_artifact_namespace_in_transaction,
        task_store::{
            create_manual_content_in_transaction, get_content_in_transaction, TaskContentSnapshot,
            TaskStoreError,
        },
    },
};

type TaskResult<T> = std::result::Result<T, SessionTaskError>;

pub(crate) struct TaskCreateWorkspace {
    task: String,
    artifacts: String,
    scope: WorkspaceRef,
}

impl TaskCreateWorkspace {
    pub(crate) fn new(task: &str, artifacts: &str, scope: WorkspaceRef) -> TaskResult<Self> {
        if !valid_namespace(task) || !valid_namespace(artifacts) || task == artifacts {
            return Err(SessionTaskError::InvalidNamespace);
        }
        Ok(Self {
            task: task.into(),
            artifacts: artifacts.into(),
            scope,
        })
    }
}

/// Opaque expected result and original guard; no serialization, replay, recovery or commit API.
pub(crate) struct PreparedTaskCreate {
    authority_id: AuthorityId,
    context: PrivateWorkContext,
    pending: TaskContentSnapshot,
}

impl PreparedTaskCreate {
    pub(crate) async fn prepare(
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
        workspace: &TaskCreateWorkspace,
        id: TaskId,
        manifest: TaskContentManifest,
    ) -> TaskResult<Self> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        if !manifest.payload().is_empty() {
            return Err(TaskStoreError::InvalidInput.into());
        }
        if authority_id != workspace.scope.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        // This mode pins Source/Registry without requiring password-wide SELECT; INSERT rights
        // and current owner authority are still independently required by the business command.
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
        if get_content_in_transaction(tx, workspace.scope, None, reference, context.owner)
            .await?
            .is_some()
        {
            return Err(TaskStoreError::Conflict.into());
        }
        // Even a zero-payload creation pins the configured Artifact namespace/schema; it does
        // not publish files or need Artifact business-write privileges.
        context.select_target(tx, &workspace.artifacts).await?;
        validate_artifact_namespace_in_transaction(tx, workspace.scope).await?;
        context.select_target(tx, &workspace.task).await?;
        let pending =
            create_manual_content_in_transaction(tx, workspace.scope, id, context.owner, manifest)
                .await?;
        // Deferred trigger work must complete BEFORE the expected pair, original target/storage
        // and final authorization checks. No business DML follows this barrier.
        sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
            .execute(&mut **tx)
            .await?;
        // Deferred work can alter the empty Task's associated Artifact namespace or business
        // table shape without changing a marker. Revalidate the original target after all DML.
        context.select_target(tx, &workspace.artifacts).await?;
        validate_artifact_namespace_in_transaction(tx, workspace.scope).await?;
        context.select_target(tx, &workspace.task).await?;
        if get_content_in_transaction(tx, workspace.scope, None, reference, context.owner)
            .await?
            .as_ref()
            != Some(&pending)
        {
            return Err(TaskStoreError::InvalidRecord.into());
        }
        // Host guards operate in the original Source, before finish's last fresh Session read.
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
    ) -> TaskResult<TaskContentSnapshot> {
        self.context
            .finish_from_source_for(self.authority_id, tx)
            .await?;
        Ok(self.pending)
    }
}
