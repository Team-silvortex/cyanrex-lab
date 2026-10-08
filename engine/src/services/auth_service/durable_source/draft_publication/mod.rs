//! Caller-owned create-only publication steps, not a batch transaction or durable journal.
//!
//! Each explicit advance performs at most one existing Session-authorized write. Its target is
//! marked unconfirmed before dispatch, so dropping the borrowed future preserves earlier receipts
//! and the uncertain target in the still-owned attempt. Dropping the attempt or process loses this
//! in-memory record. No automatic continuation, retry, cleanup, installer or recovery is provided.
use super::{
    valid_token, DurableAuthSource, SessionArtifactError, SessionTaskContentWorkspace,
    SessionTaskError,
};
use crate::{
    models::collaboration::{ArtifactRef, ArtifactRevision, TaskRef},
    services::{task_draft_import::TaskDraftPublicationPlan, task_store::TaskContentSnapshot},
};
use sha2::{Digest, Sha256};

mod checkpoint;
pub use checkpoint::{
    SessionDraftCheckpointState, SessionDraftPublicationCheckpoint,
    MAX_SESSION_DRAFT_CHECKPOINT_BYTES,
};
mod confirmation;
mod dispatch;
pub use dispatch::{SessionDraftDispatchError, SessionJournaledTaskDraftPublication};
mod dispatch_transaction;
mod intent;
pub use intent::{
    SessionDraftIntentError, SessionDraftIntentWorkspace, SessionDraftPublicationIntent,
};
mod journal_inspection;
pub use journal_inspection::SessionDraftJournalInspection;
mod journal_observation;
pub use journal_observation::{
    SessionDraftJournalObservationError, SessionDraftJournalObservedOutcome,
    SessionDraftJournalStepObservation, SessionDraftRecordedStepStatus,
};
mod inspection;
pub use inspection::{
    SessionDraftCheckpointInspectionError, SessionDraftCheckpointObservation,
    SessionDraftCheckpointObservedOutcome,
};
mod observation;
pub use observation::{SessionDraftObservedOutcome, SessionDraftPublicationObservation};

/// Metadata only, never a reusable authorization grant or proof of current storage state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionDraftPublicationStep {
    Artifact {
        index: usize,
        reference: ArtifactRef,
    },
    Task {
        reference: TaskRef,
    },
}

/// Unconfirmed is terminal even when an underlying error appears to describe an early rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionDraftPublicationState {
    Ready,
    Unconfirmed(SessionDraftPublicationStep),
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDraftPublicationProgress {
    ArtifactConfirmed { index: usize },
    TaskConfirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionDraftPublicationError {
    #[error("invalid draft publication preparation")]
    InvalidPreparation,
    #[error("draft publication requires its original session token")]
    SessionChanged,
    #[error("draft publication cannot dispatch another operation")]
    Stopped,
    #[error("draft publication has no unconfirmed step to observe")]
    NoUnconfirmedStep,
    #[error("draft publication did not confirm the expected snapshot")]
    InvalidConfirmation,
    #[error(transparent)]
    Artifact(#[from] SessionArtifactError),
    #[error(transparent)]
    Task(#[from] SessionTaskError),
}

type PublicationResult<T> = Result<T, SessionDraftPublicationError>;

/// Opaque caller-owned state; intentionally no Clone, Debug, Serialize or Deserialize.
/// The token fingerprint only prevents accidental Session substitution; it does not authenticate.
pub struct SessionTaskDraftPublication {
    source: DurableAuthSource,
    workspace: SessionTaskContentWorkspace,
    plan: TaskDraftPublicationPlan,
    fingerprint: [u8; 32],
    task: TaskRef,
    allocated: Vec<ArtifactRef>,
    confirmed: Vec<ArtifactRevision>,
    completed: Option<TaskContentSnapshot>,
    state: SessionDraftPublicationState,
}

fn fingerprint(token: &str) -> [u8; 32] {
    Sha256::new()
        .chain_update(b"cyanrex.task-draft-publication.session.v1\0")
        .chain_update(token.as_bytes())
        .finalize()
        .into()
}

impl DurableAuthSource {
    /// Local preparation only. Server targets are allocated before any write; local draft IDs
    /// and revisions cannot choose them. Authority agreement is structural, not authentication.
    pub fn prepare_task_draft_publication(
        &self,
        token: &str,
        workspace: SessionTaskContentWorkspace,
        plan: TaskDraftPublicationPlan,
    ) -> PublicationResult<SessionTaskDraftPublication> {
        let scope = workspace.publication_scope();
        if !valid_token(token) || scope.authority_id != self.authority_id {
            return Err(SessionDraftPublicationError::InvalidPreparation);
        }
        let task = TaskRef {
            workspace: scope,
            task_id: uuid::Uuid::new_v4()
                .try_into()
                .map_err(|_| SessionDraftPublicationError::InvalidPreparation)?,
        };
        let mut allocated: Vec<ArtifactRef> = Vec::with_capacity(plan.items().len());
        for item in plan.items() {
            let reference = ArtifactRef {
                workspace: scope,
                artifact_id: uuid::Uuid::new_v4()
                    .try_into()
                    .map_err(|_| SessionDraftPublicationError::InvalidPreparation)?,
                revision_id: uuid::Uuid::new_v4()
                    .try_into()
                    .map_err(|_| SessionDraftPublicationError::InvalidPreparation)?,
                sha256: format!("{:x}", Sha256::digest(item.text().as_bytes()))
                    .parse()
                    .map_err(|_| SessionDraftPublicationError::InvalidPreparation)?,
            };
            if allocated.iter().any(|other| {
                other.artifact_id == reference.artifact_id
                    || other.revision_id == reference.revision_id
            }) {
                return Err(SessionDraftPublicationError::InvalidPreparation);
            }
            allocated.push(reference);
        }
        Ok(SessionTaskDraftPublication {
            source: self.clone(),
            workspace,
            plan,
            fingerprint: fingerprint(token),
            task,
            confirmed: Vec::with_capacity(allocated.len()),
            allocated,
            completed: None,
            state: SessionDraftPublicationState::Ready,
        })
    }
}

impl SessionTaskDraftPublication {
    pub fn state(&self) -> &SessionDraftPublicationState {
        &self.state
    }

    pub fn task_ref(&self) -> TaskRef {
        self.task
    }

    /// Planned exact targets, not published records or permissions to read existing content.
    pub fn allocated_artifacts(&self) -> &[ArtifactRef] {
        &self.allocated
    }

    /// Confirmed command results only, never current authorization or a promise bytes still exist.
    pub fn confirmed_artifacts(&self) -> &[ArtifactRevision] {
        &self.confirmed
    }

    pub fn confirmed_task(&self) -> Option<&TaskContentSnapshot> {
        self.completed.as_ref()
    }

    /// Advances at most one write. An unpolled future does nothing. A wrong token is rejected
    /// before marking, leaving a ready attempt usable with its original token. After dispatch,
    /// every error or dropped future leaves the exact step terminally unconfirmed; do not retry.
    /// No total workflow deadline is imposed; each existing command retains its own deadline.
    pub async fn advance(
        &mut self,
        token: &str,
    ) -> PublicationResult<SessionDraftPublicationProgress> {
        if self.state != SessionDraftPublicationState::Ready {
            return Err(SessionDraftPublicationError::Stopped);
        }
        if !valid_token(token) || fingerprint(token) != self.fingerprint {
            return Err(SessionDraftPublicationError::SessionChanged);
        }
        let source = self.source.clone();
        let index = self.confirmed.len();
        if index < self.allocated.len() {
            let artifacts = self.workspace.publication_artifacts().clone();
            let target = self.allocated[index].clone();
            let draft = self
                .plan
                .publication_draft(index)
                .map_err(|_| SessionDraftPublicationError::InvalidPreparation)?;
            self.await_artifact(
                index,
                source.create_session_artifact(
                    token,
                    &artifacts,
                    target.artifact_id,
                    target.revision_id,
                    draft,
                ),
            )
            .await
        } else {
            let manifest = self.publication_manifest()?;
            let workspace = self.workspace.clone();
            let expected = manifest.clone();
            self.await_task(
                expected,
                source.create_session_content_task(token, &workspace, self.task.task_id, manifest),
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests;
