use super::*;
use crate::models::collaboration::{ArtifactContent, TaskContentManifest};
use crate::services::auth_service::durable_source::SessionTaskContent;

/// Agreement at one authorized read, not the outcome or provenance of the previous write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDraftObservedOutcome {
    MatchesPlannedCreate,
    DiffersFromPlannedCreate,
    /// Absent or not visible to the current Session. Does not establish rollback or reserve IDs.
    NotVisible,
}

/// No content or credential is retained. IDs/digests are still private workflow metadata.
/// This observation never resumes the attempt or changes its historical confirmation receipts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDraftPublicationObservation {
    pub step: SessionDraftPublicationStep,
    pub result: SessionDraftObservedOutcome,
}

impl SessionTaskDraftPublication {
    /// Observes only the terminally uncertain step, using its original token and one existing
    /// current-Session read transaction. The fingerprint is not an authorization precheck.
    /// No prefix scan, adoption, recovery, retry or deletion follows any result or read error.
    /// Artifact reads may find an exact historical revision. Task reads inspect the current Task;
    /// a later edit can differ without establishing whether this earlier create ever committed.
    /// Missing Task reads validate source + Task scope only, not the Artifact namespace/prefix.
    /// The existing locking readers are not SQL READ ONLY transactions. A successful read can
    /// immediately become stale; NotVisible neither proves rollback nor prevents a later commit.
    pub async fn observe_unconfirmed(
        &self,
        token: &str,
    ) -> PublicationResult<SessionDraftPublicationObservation> {
        let SessionDraftPublicationState::Unconfirmed(step) = &self.state else {
            return Err(SessionDraftPublicationError::NoUnconfirmedStep);
        };
        if !valid_token(token) || fingerprint(token) != self.fingerprint {
            return Err(SessionDraftPublicationError::SessionChanged);
        }
        let result = match step {
            SessionDraftPublicationStep::Artifact { index, reference } => {
                let content = self
                    .source
                    .read_session_artifact(token, self.workspace.publication_artifacts(), reference)
                    .await?;
                self.artifact_observation(*index, content.as_ref())
            }
            SessionDraftPublicationStep::Task { reference } => {
                let expected = self.publication_manifest()?;
                let content = self
                    .source
                    .get_session_content_task(token, &self.workspace, reference.task_id)
                    .await?;
                self.task_observation(&expected, content.as_ref())
            }
        };
        Ok(SessionDraftPublicationObservation {
            step: step.clone(),
            result,
        })
    }

    pub(super) fn artifact_observation(
        &self,
        index: usize,
        content: Option<&ArtifactContent>,
    ) -> SessionDraftObservedOutcome {
        let Some(content) = content else {
            return SessionDraftObservedOutcome::NotVisible;
        };
        if self.artifact_matches_planned_create(index, &content.revision)
            && content.bytes == self.plan.items()[index].text().as_bytes()
        {
            SessionDraftObservedOutcome::MatchesPlannedCreate
        } else {
            SessionDraftObservedOutcome::DiffersFromPlannedCreate
        }
    }

    pub(super) fn task_observation(
        &self,
        expected: &TaskContentManifest,
        content: Option<&SessionTaskContent>,
    ) -> SessionDraftObservedOutcome {
        let Some(content) = content else {
            return SessionDraftObservedOutcome::NotVisible;
        };
        if self.task_matches_planned_create(expected, &content.snapshot)
            && content.contents.len() == self.plan.items().len()
            && content.contents.iter().enumerate().all(|(index, item)| {
                self.artifact_observation(index, Some(item))
                    == SessionDraftObservedOutcome::MatchesPlannedCreate
            })
        {
            SessionDraftObservedOutcome::MatchesPlannedCreate
        } else {
            SessionDraftObservedOutcome::DiffersFromPlannedCreate
        }
    }
}
