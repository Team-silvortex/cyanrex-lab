//! Current authorization of a caller-supplied metadata target, not a restored publication attempt.
use super::*;
use crate::{
    models::collaboration::{ArtifactContent, TaskStatus},
    services::{
        auth_service::durable_source::SessionTaskContent,
        task_content::validate_text_payload_content,
    },
};

/// Agreement with an untrusted metadata claim, not original-body equality or write provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDraftCheckpointObservedOutcome {
    MatchesCheckpointMetadata,
    DiffersFromCheckpointMetadata,
    /// This authorized read found no visible target; neither rollback nor safe retry follows.
    NotVisible,
}

/// Body-free metadata, which must still remain private. No imported confirmation or new authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDraftCheckpointObservation {
    pub step: SessionDraftPublicationStep,
    pub result: SessionDraftCheckpointObservedOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionDraftCheckpointInspectionError {
    #[error("checkpoint has no reported unconfirmed step")]
    NoUnconfirmedStep,
    #[error("checkpoint scope does not match the configured workspace and source")]
    ScopeMismatch,
    #[error(transparent)]
    Artifact(#[from] SessionArtifactError),
    #[error(transparent)]
    Task(#[from] SessionTaskError),
}

impl DurableAuthSource {
    /// Reads only the reported uncertain target using trusted routing and the current Session.
    /// No original token or surviving attempt is required or reconstructed. Each existing reader
    /// derives the current owner and confirms its final Session check/commit before data returns.
    /// The checkpoint cannot identify the original owner, storage incarnation or prior outcome.
    /// A missing Task validates source + Task only; it does not validate the Artifact namespace.
    /// Existing readers take locks, are not SQL READ ONLY, and retain their own bounds/deadlines.
    /// No result or cancellation updates a checkpoint, records a receipt or allows a retry.
    pub async fn inspect_task_draft_checkpoint(
        &self,
        token: &str,
        workspace: &SessionTaskContentWorkspace,
        checkpoint: &SessionDraftPublicationCheckpoint,
    ) -> Result<SessionDraftCheckpointObservation, SessionDraftCheckpointInspectionError> {
        let step = checkpoint
            .reported_unconfirmed_step()
            .ok_or(SessionDraftCheckpointInspectionError::NoUnconfirmedStep)?;
        let scope = workspace.publication_scope();
        if checkpoint.task_ref().workspace != scope || scope.authority_id != self.authority_id {
            return Err(SessionDraftCheckpointInspectionError::ScopeMismatch);
        }
        let result = match &step {
            SessionDraftPublicationStep::Artifact { index, reference } => {
                let content = self
                    .read_session_artifact(token, workspace.publication_artifacts(), reference)
                    .await?;
                artifact_outcome(checkpoint, *index, content.as_ref())
            }
            SessionDraftPublicationStep::Task { reference } => {
                let content = self
                    .get_session_content_task(token, workspace, reference.task_id)
                    .await?;
                task_outcome(checkpoint, content.as_ref())
            }
        };
        Ok(SessionDraftCheckpointObservation { step, result })
    }
}

pub(super) fn artifact_outcome(
    checkpoint: &SessionDraftPublicationCheckpoint,
    index: usize,
    content: Option<&ArtifactContent>,
) -> SessionDraftCheckpointObservedOutcome {
    let Some(content) = content else {
        return SessionDraftCheckpointObservedOutcome::NotVisible;
    };
    if artifact_matches_metadata(checkpoint, index, content) {
        SessionDraftCheckpointObservedOutcome::MatchesCheckpointMetadata
    } else {
        SessionDraftCheckpointObservedOutcome::DiffersFromCheckpointMetadata
    }
}

fn artifact_matches_metadata(
    checkpoint: &SessionDraftPublicationCheckpoint,
    index: usize,
    content: &ArtifactContent,
) -> bool {
    let Some(binding) = checkpoint.manifest().payload().get(index) else {
        return false;
    };
    let Some(length) = checkpoint.byte_lengths().get(index) else {
        return false;
    };
    let revision = &content.revision;
    revision.reference == *binding.artifact()
        && revision.owner.authority_id == checkpoint.task_ref().workspace.authority_id
        && u64::from(revision.sequence) == 1
        && revision.parent.is_none()
        && revision.kind.as_str() == "cyanrex.task-text"
        && revision.title == checkpoint.manifest().title()
        && revision.media_type == "text/plain"
        && revision.byte_length == *length
        // This owner came from the authorized reader, not the record. The helper checks only
        // consistency/text shape; it must never become a detached authorization precheck.
        && validate_text_payload_content(binding, revision.owner, content).is_ok()
}

pub(super) fn task_outcome(
    checkpoint: &SessionDraftPublicationCheckpoint,
    content: Option<&SessionTaskContent>,
) -> SessionDraftCheckpointObservedOutcome {
    let Some(content) = content else {
        return SessionDraftCheckpointObservedOutcome::NotVisible;
    };
    let snapshot = &content.snapshot;
    let task = &snapshot.task;
    if task.reference == checkpoint.task_ref()
        && snapshot.manifest == *checkpoint.manifest()
        && task.owner.authority_id == checkpoint.task_ref().workspace.authority_id
        && task.definition.is_none()
        && task.status == TaskStatus::Draft
        && u64::from(task.revision) == 1
        && snapshot.manifest.validate_task_snapshot(task).is_ok()
        && content.contents.len() == checkpoint.manifest().payload().len()
        && content.contents.iter().enumerate().all(|(index, item)| {
            item.revision.owner == task.owner && artifact_matches_metadata(checkpoint, index, item)
        })
    {
        SessionDraftCheckpointObservedOutcome::MatchesCheckpointMetadata
    } else {
        SessionDraftCheckpointObservedOutcome::DiffersFromCheckpointMetadata
    }
}
