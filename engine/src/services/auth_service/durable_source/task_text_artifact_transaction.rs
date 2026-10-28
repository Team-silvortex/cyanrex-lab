//! One Task-first guard and one source transaction, not a detached GET followed by publication.
use super::{
    private_work::{valid_namespace, PrivateWorkContext},
    text_artifact_transaction::TextArtifactPublishWorkspace,
    *,
};
use crate::{
    models::collaboration::{
        ArtifactId, ArtifactRevision, ArtifactRevisionId, TaskStatus, WorkspaceRef,
    },
    services::{
        artifact_store::{ArtifactContentWriter, ArtifactDraft},
        task_store::{get_content_in_transaction, TaskContentSnapshot, TaskStoreError},
    },
};

type Result<T> = std::result::Result<T, SessionTaskError>;

pub(crate) struct TaskTextArtifactPublishWorkspace {
    task: String,
    artifacts: String,
    scope: WorkspaceRef,
    writer: ArtifactContentWriter,
}

impl TaskTextArtifactPublishWorkspace {
    pub(crate) fn new(
        task: &str,
        artifacts: &str,
        scope: WorkspaceRef,
        writer: ArtifactContentWriter,
    ) -> Result<Self> {
        if !valid_namespace(task) || !valid_namespace(artifacts) || task == artifacts {
            return Err(SessionTaskError::InvalidNamespace);
        }
        if writer.scope() != scope {
            return Err(TaskStoreError::ScopeMismatch.into());
        }
        Ok(Self {
            task: task.into(),
            artifacts: artifacts.into(),
            scope,
            writer,
        })
    }

    /// Only structural/profile checks before I/O. Actual ownership is derived in prepare.
    pub(crate) fn draft(
        scope: WorkspaceRef,
        expected: &TaskContentSnapshot,
        text: String,
    ) -> Result<ArtifactDraft> {
        if expected.task.reference.workspace != scope
            || expected.task.owner.authority_id != scope.authority_id
            || expected
                .manifest
                .payload()
                .iter()
                .any(|item| item.artifact().workspace != scope)
        {
            return Err(TaskStoreError::ScopeMismatch.into());
        }
        if expected.task.definition.is_some() {
            return Err(SessionTaskError::UnsupportedTask);
        }
        expected
            .manifest
            .validate_task_snapshot(&expected.task)
            .map_err(|_| TaskStoreError::InvalidInput)?;
        TextArtifactPublishWorkspace::draft(expected.task.title.clone(), text).map_err(|error| {
            match error {
                SessionArtifactError::Session(error) => SessionTaskError::Session(error),
                SessionArtifactError::Artifact(error) => SessionTaskError::Artifact(error),
                SessionArtifactError::InvalidNamespace => SessionTaskError::InvalidNamespace,
            }
        })
    }

    pub(crate) async fn verify_root(&self) -> Result<()> {
        Ok(self.writer.verify_root().await?)
    }
}

pub(crate) struct PreparedTaskTextArtifactPublish {
    authority_id: AuthorityId,
    context: PrivateWorkContext,
    pending: ArtifactRevision,
}

impl PreparedTaskTextArtifactPublish {
    pub(crate) async fn prepare(
        authority_id: AuthorityId,
        tx: &mut Transaction<'_, Postgres>,
        token: &str,
        workspace: &TaskTextArtifactPublishWorkspace,
        expected: TaskContentSnapshot,
        artifact: ArtifactId,
        revision: ArtifactRevisionId,
        draft: ArtifactDraft,
    ) -> Result<Self> {
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
        // The baseline owner is only a consistency pin. Never dispatch bytes into a different
        // current actor's account, even if an old page survives a cross-tab Cookie change.
        if context.owner != expected.task.owner {
            return Err(TaskStoreError::NotFound.into());
        }
        let reference = expected.task.reference;
        let old = get_content_in_transaction(tx, workspace.scope, None, reference, context.owner)
            .await?
            .ok_or(TaskStoreError::NotFound)?;
        if old.task.revision != expected.task.revision {
            return Err(TaskStoreError::StaleRevision.into());
        }
        if old.task.status != TaskStatus::Draft {
            return Err(TaskStoreError::InvalidTransition.into());
        }
        if old != expected {
            return Err(TaskStoreError::InvalidInput.into());
        }
        // A Task lock wait may have consumed the original Session's remaining lifetime.
        context.recheck_selected_for(authority_id, tx).await?;
        context.select_target(tx, &workspace.artifacts).await?;
        let pending = workspace
            .writer
            .create_in_transaction(tx, artifact, revision, context.owner, draft)
            .await?;
        sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
            .execute(&mut **tx)
            .await?;
        workspace.writer.verify_in_transaction(tx, &pending).await?;
        context.select_target(tx, &workspace.task).await?;
        if get_content_in_transaction(tx, workspace.scope, None, reference, context.owner)
            .await?
            .as_ref()
            != Some(&old)
        {
            return Err(TaskStoreError::InvalidRecord.into());
        }
        // This guard checks Task metadata, not old payload bytes or a completed Task save.
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
    ) -> Result<ArtifactRevision> {
        self.context
            .finish_from_source_for(self.authority_id, tx)
            .await?;
        Ok(self.pending)
    }
}
