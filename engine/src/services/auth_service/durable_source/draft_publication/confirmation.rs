use super::*;
use crate::models::collaboration::{ArtifactContent, TaskContentManifest, TaskStatus};
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
        let item = &self.plan.items()[index];
        revision.reference == self.allocated[index]
            && revision.owner.authority_id == self.task.workspace.authority_id
            && self
                .confirmed
                .first()
                .is_none_or(|first| first.owner == revision.owner)
            && u64::from(revision.sequence) == 1
            && revision.parent.is_none()
            && revision.kind.as_str() == "cyanrex.task-text"
            && revision.title == self.plan.title()
            && revision.media_type == "text/plain"
            && revision.byte_length == item.text().len() as u64
    }

    pub(super) fn publication_manifest(&self) -> PublicationResult<TaskContentManifest> {
        if self.confirmed.is_empty() && self.plan.items().is_empty() {
            // No placeholder owner: the Task command derives the actual owner in its transaction.
            return TaskContentManifest::new(self.plan.title(), vec![])
                .map_err(|_| SessionDraftPublicationError::InvalidPreparation);
        }
        let owner = self
            .confirmed
            .first()
            .ok_or(SessionDraftPublicationError::InvalidConfirmation)?
            .owner;
        let contents: Vec<_> = self
            .confirmed
            .iter()
            .zip(self.plan.items())
            .map(|(revision, item)| ArtifactContent {
                revision: revision.clone(),
                bytes: item.text().as_bytes().to_vec(),
            })
            .collect();
        // Original bytes plus confirmed metadata, not a fresh Artifact read. C2-M must still
        // verify actual stored bytes and the current Session inside the Task write transaction.
        self.plan
            .bind_published(self.task.workspace, owner, &contents)
            .map_err(|_| SessionDraftPublicationError::InvalidConfirmation)
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
