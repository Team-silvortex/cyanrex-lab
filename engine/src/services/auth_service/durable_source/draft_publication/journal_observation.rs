//! One journal-bound current resource read, never a publication receipt or recovery grant.
use super::super::{private_work::PrivateWorkError, DurableAuthError};
use super::*;
use crate::models::collaboration::TaskId;
use crate::{services::draft_intent_journal::DraftIntentJournalError, sqlx_compat as sqlx};

type ObservationResult<T> = Result<T, SessionDraftJournalObservationError>;
impl From<DraftIntentJournalError> for SessionDraftJournalObservationError {
    fn from(error: DraftIntentJournalError) -> Self {
        Self::Intent(error.into())
    }
}
impl From<PrivateWorkError> for SessionDraftJournalObservationError {
    fn from(error: PrivateWorkError) -> Self {
        Self::Intent(error.into())
    }
}
impl From<DurableAuthError> for SessionDraftJournalObservationError {
    fn from(error: DurableAuthError) -> Self {
        Self::Intent(error.into())
    }
}
impl From<sqlx::Error> for SessionDraftJournalObservationError {
    fn from(_: sqlx::Error) -> Self {
        DraftIntentJournalError::StorageUnavailable.into()
    }
}
async fn bounded<T>(
    operation: impl std::future::Future<Output = ObservationResult<T>>,
) -> ObservationResult<T> {
    tokio::time::timeout(std::time::Duration::from_secs(10), operation)
        .await
        .map_err(|_| {
            SessionDraftJournalObservationError::from(DraftIntentJournalError::StorageUnavailable)
        })?
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDraftRecordedStepStatus {
    Unknown,
    Committed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDraftJournalObservedOutcome {
    MatchesIntentMetadata,
    DiffersFromIntentMetadata,
    NotVisible,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionDraftJournalObservationError {
    #[error(transparent)]
    Intent(#[from] SessionDraftIntentError),
    #[error(transparent)]
    Artifact(#[from] SessionArtifactError),
    #[error(transparent)]
    Task(#[from] SessionTaskError),
    #[error("journal step ordinal exceeds the maximum")]
    InvalidOrdinal,
    #[error("journal has no recorded step at this ordinal")]
    NoRecordedStep,
}
/// Private metadata only. No nonce, content, Serde, Clone or conversion into a live attempt.
pub struct SessionDraftJournalStepObservation {
    step: SessionDraftPublicationStep,
    recorded_status: SessionDraftRecordedStepStatus,
    result: SessionDraftJournalObservedOutcome,
}
impl SessionDraftJournalStepObservation {
    pub fn step(&self) -> &SessionDraftPublicationStep {
        &self.step
    }
    pub fn recorded_status(&self) -> SessionDraftRecordedStepStatus {
        self.recorded_status
    }
    pub fn result(&self) -> SessionDraftJournalObservedOutcome {
        self.result
    }
}
impl DurableAuthSource {
    /// One recorded target at the current read time, not a resource receipt or restored attempt.
    /// NoRecordedStep and None also require the final current-Session check and confirmed commit.
    pub async fn observe_session_draft_journal_step(
        &self,
        token: &str,
        workspace: &SessionDraftIntentWorkspace,
        task: TaskId,
        ordinal: usize,
    ) -> Result<Option<SessionDraftJournalStepObservation>, SessionDraftJournalObservationError>
    {
        if ordinal > 32 {
            return Err(SessionDraftJournalObservationError::InvalidOrdinal);
        }
        if !valid_token(token) {
            return Err(SessionDraftIntentError::from(
                super::super::SessionCommandError::InvalidSession,
            )
            .into());
        }
        bounded(async {
            let mut tx = self.transaction().await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            let mut context = self
                .begin_private_work(
                    &mut tx,
                    token,
                    &workspace.journal_namespace,
                    workspace.content.publication_scope(),
                )
                .await?;
            workspace.check_routes(context.source_namespace())?;
            let reference = TaskRef {
                workspace: workspace.content.publication_scope(),
                task_id: task,
            };
            let snapshot = workspace
                .journal
                .capture_in_transaction(&mut tx, reference, context.owner, context.account_id())
                .await?;
            let pending = if let Some(snapshot) = snapshot {
                workspace.check_record_routes(snapshot.record(), context.source_namespace())?;
                let checkpoint = snapshot.record().validate()?;
                let pending = if let Some(recorded) = snapshot.steps().get(ordinal) {
                    let (step, result) = if let Some(item) =
                        checkpoint.manifest().payload().get(ordinal)
                    {
                        let artifacts = workspace.content.publication_artifacts();
                        context.select_target(&mut tx, &artifacts.namespace).await?;
                        let content = artifacts
                            .store
                            .read_in_transaction(&mut tx, item.artifact(), context.owner)
                            .await
                            .map_err(SessionArtifactError::from)?;
                        (
                            SessionDraftPublicationStep::Artifact {
                                index: ordinal,
                                reference: item.artifact().clone(),
                            },
                            inspection::artifact_outcome(&checkpoint, ordinal, content.as_ref()),
                        )
                    } else {
                        context
                            .select_target(&mut tx, workspace.content.publication_namespace())
                            .await?;
                        let content = self
                            .read_content_task_in_transaction(
                                &mut tx,
                                &mut context,
                                &workspace.content,
                                task,
                            )
                            .await?;
                        (
                            SessionDraftPublicationStep::Task { reference },
                            inspection::task_outcome(&checkpoint, content.as_ref()),
                        )
                    };
                    let result = match result {
                        SessionDraftCheckpointObservedOutcome::MatchesCheckpointMetadata => {
                            SessionDraftJournalObservedOutcome::MatchesIntentMetadata
                        }
                        SessionDraftCheckpointObservedOutcome::DiffersFromCheckpointMetadata => {
                            SessionDraftJournalObservedOutcome::DiffersFromIntentMetadata
                        }
                        SessionDraftCheckpointObservedOutcome::NotVisible => {
                            SessionDraftJournalObservedOutcome::NotVisible
                        }
                    };
                    Ok(Some(SessionDraftJournalStepObservation {
                        step,
                        recorded_status: if recorded.committed {
                            SessionDraftRecordedStepStatus::Committed
                        } else {
                            SessionDraftRecordedStepStatus::Unknown
                        },
                        result,
                    }))
                } else {
                    Err(SessionDraftJournalObservationError::NoRecordedStep)
                };
                context
                    .select_target(&mut tx, &workspace.journal_namespace)
                    .await?;
                snapshot.verify(&workspace.journal, &mut tx).await?;
                pending
            } else {
                Ok(None)
            };
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            pending
        })
        .await
    }
}
