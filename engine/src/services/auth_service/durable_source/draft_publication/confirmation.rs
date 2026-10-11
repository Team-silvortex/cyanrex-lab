use super::super::draft_publication_checks;
use super::*;
use crate::models::collaboration::{TaskContentManifest, TaskStatus};
use std::future::Future;

impl SessionTaskDraftPublication {
    // These private helpers also let unit tests exercise the exact pre-await receipt boundary
    // with controlled futures. Production always supplies the real Session command, never a hook.
    pub(super) async fn await_artifact(
        &mut self,
        index: usize,
        command: impl Future<Output = Result<ArtifactRevision, SessionArtifactError>>,
    ) -> PublicationResult<SessionDraftPublicationProgress> {
        self.state =
            SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Artifact {
                index,
                reference: self.allocated[index].clone(),
            });
        let revision = command.await?;
        if !self.artifact_matches_planned_create(index, &revision) {
            return Err(SessionDraftPublicationError::InvalidConfirmation);
        }
        // No awaited work after commit acknowledgement and before preserving its full result.
        self.confirmed.push(revision);
        self.state = SessionDraftPublicationState::Ready;
        Ok(SessionDraftPublicationProgress::ArtifactConfirmed { index })
    }

    pub(super) fn artifact_matches_planned_create(
        &self,
        index: usize,
        revision: &ArtifactRevision,
    ) -> bool {
        draft_publication_checks::artifact_matches_planned_create(
            &self.plan,
            self.task,
            &self.allocated,
            &self.confirmed,
            index,
            revision,
        )
    }

    pub(super) fn publication_manifest(&self) -> PublicationResult<TaskContentManifest> {
        draft_publication_checks::publication_manifest(&self.plan, self.task, &self.confirmed)
    }

    pub(super) async fn await_task(
        &mut self,
        expected: TaskContentManifest,
        command: impl Future<Output = Result<TaskContentSnapshot, SessionTaskError>>,
    ) -> PublicationResult<SessionDraftPublicationProgress> {
        self.state = SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Task {
            reference: self.task,
        });
        let snapshot = command.await?;
        if !self.task_matches_planned_create(&expected, &snapshot) {
            return Err(SessionDraftPublicationError::InvalidConfirmation);
        }
        self.completed = Some(snapshot);
        self.state = SessionDraftPublicationState::Complete;
        Ok(SessionDraftPublicationProgress::TaskConfirmed)
    }

    pub(super) fn task_matches_planned_create(
        &self,
        expected: &TaskContentManifest,
        snapshot: &TaskContentSnapshot,
    ) -> bool {
        snapshot.manifest == *expected
            && snapshot.task.reference == self.task
            && snapshot.task.owner.authority_id == self.task.workspace.authority_id
            && self
                .confirmed
                .first()
                .is_none_or(|first| first.owner == snapshot.task.owner)
            && snapshot.task.definition.is_none()
            && snapshot.task.status == TaskStatus::Draft
            && u64::from(snapshot.task.revision) == 1
            && snapshot
                .manifest
                .validate_task_snapshot(&snapshot.task)
                .is_ok()
    }
}
