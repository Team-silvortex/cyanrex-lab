//! An explicitly consumed original attempt, not imported recovery or a retry surface.
use super::dispatch_transaction::{DispatchOperation, DispatchPending};
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionDraftDispatchError {
    #[error(transparent)]
    Publication(#[from] SessionDraftPublicationError),
    #[error(transparent)]
    Intent(#[from] SessionDraftIntentError),
    #[error(transparent)]
    Artifact(#[from] SessionArtifactError),
    #[error(transparent)]
    Task(#[from] SessionTaskError),
}

/// Owns the sole live attempt. No Clone, importer, unwrap or reusable execution grant.
pub struct SessionJournaledTaskDraftPublication {
    pub(super) attempt: SessionTaskDraftPublication,
    pub(super) workspace: SessionDraftIntentWorkspace,
    pub(super) checkpoint: Vec<u8>,
}

impl SessionTaskDraftPublication {
    /// Pure, pristine conversion only. Registration and current authorization remain separate.
    pub fn into_journaled(
        self,
        workspace: SessionDraftIntentWorkspace,
    ) -> Result<SessionJournaledTaskDraftPublication, SessionDraftIntentError> {
        if self.state != SessionDraftPublicationState::Ready
            || !self.confirmed.is_empty()
            || self.completed.is_some()
            || !workspace.matches_attempt(&self)
        {
            return Err(SessionDraftIntentError::InvalidAttempt);
        }
        let checkpoint = self
            .checkpoint()
            .and_then(|c| c.to_json())
            .map_err(|_| SessionDraftIntentError::InvalidAttempt)?;
        Ok(SessionJournaledTaskDraftPublication {
            attempt: self,
            workspace,
            checkpoint,
        })
    }
}

impl SessionJournaledTaskDraftPublication {
    pub fn state(&self) -> &SessionDraftPublicationState {
        self.attempt.state()
    }
    pub fn task_ref(&self) -> TaskRef {
        self.attempt.task_ref()
    }
    pub fn allocated_artifacts(&self) -> &[ArtifactRef] {
        self.attempt.allocated_artifacts()
    }
    pub fn confirmed_artifacts(&self) -> &[ArtifactRevision] {
        self.attempt.confirmed_artifacts()
    }
    pub fn confirmed_task(&self) -> Option<&TaskContentSnapshot> {
        self.attempt.confirmed_task()
    }
    pub fn checkpoint(
        &self,
    ) -> Result<SessionDraftPublicationCheckpoint, crate::models::collaboration::ContractError>
    {
        self.attempt.checkpoint()
    }
    pub async fn advance(
        &mut self,
        token: &str,
    ) -> Result<SessionDraftPublicationProgress, SessionDraftDispatchError> {
        if self.attempt.state != SessionDraftPublicationState::Ready {
            return Err(SessionDraftPublicationError::Stopped.into());
        }
        if !valid_token(token) || fingerprint(token) != self.attempt.fingerprint {
            return Err(SessionDraftPublicationError::SessionChanged.into());
        }
        let ordinal = self.attempt.confirmed.len();
        let operation = if ordinal < self.attempt.allocated.len() {
            let draft = self
                .attempt
                .plan
                .publication_draft(ordinal)
                .map_err(|_| SessionDraftPublicationError::InvalidPreparation)?;
            self.attempt.state =
                SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Artifact {
                    index: ordinal,
                    reference: self.attempt.allocated[ordinal].clone(),
                });
            DispatchOperation::Artifact(ordinal, draft)
        } else {
            let manifest = self.attempt.publication_manifest()?;
            self.attempt.state =
                SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Task {
                    reference: self.attempt.task,
                });
            DispatchOperation::Task(manifest)
        };
        // No resource command/future is started unless phase A acknowledges its own COMMIT.
        let permit = self.reserve_dispatch(token, ordinal).await?;
        let pending = self.execute_dispatch(token, permit, operation).await?;
        // No awaits between business COMMIT acknowledgement and saving the original receipt.
        match pending {
            DispatchPending::Artifact(revision) => {
                self.attempt.confirmed.push(revision);
                self.attempt.state = SessionDraftPublicationState::Ready;
                Ok(SessionDraftPublicationProgress::ArtifactConfirmed { index: ordinal })
            }
            DispatchPending::Task(snapshot) => {
                self.attempt.completed = Some(snapshot);
                self.attempt.state = SessionDraftPublicationState::Complete;
                Ok(SessionDraftPublicationProgress::TaskConfirmed)
            }
        }
    }
}
