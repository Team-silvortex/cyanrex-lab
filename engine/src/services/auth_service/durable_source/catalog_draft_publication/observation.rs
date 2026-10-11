//! Fixed catalogue-attempt target observation, never recovered progress or write authority.
use super::super::{
    SessionDraftObservedOutcome, SessionDraftPublicationObservation, SessionDraftPublicationStep,
    SessionTaskContent,
};
use super::*;
use crate::models::collaboration::{ArtifactContent, TaskContentManifest};

impl SessionCatalogTaskDraftPublication {
    /// Reads only this surviving attempt's exact unknown target with its original Session.
    /// A matching local fingerprint is not authority: the existing command performs the current
    /// owner, membership, namespace and final Session checks in its own transaction.
    ///
    /// No read result, failure or cancellation changes the attempt, imports a receipt or allows
    /// retry. An Artifact may match an exact historical revision; a Task read observes current
    /// contents. A missing Task validates source and Task scope, not the Artifact namespace or
    /// historical prefix. Existing locking reads are not SQL READ ONLY transactions, and their
    /// per-command deadlines remain unchanged. Every result can immediately become stale.
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
                    .get_session_catalog_content_task(token, &self.workspace, reference.task_id)
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
            && self
                .plan
                .items()
                .get(index)
                .is_some_and(|item| content.bytes == item.text().as_bytes())
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
        // The real catalogue reader rejects unregistered, manual or same-reference metadata
        // drift as a typed error before reaching this comparison. Never convert those errors
        // into a report or fall back to a manual reader.
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
