//! Explicit catalogue draft preparation, independent from the manual checkpoint/journal workflow.
use super::{
    valid_token, DurableAuthSource, SessionCatalogTaskContentWorkspace,
    SessionDraftPublicationError, SessionDraftPublicationProgress, SessionDraftPublicationState,
};

use crate::{
    models::collaboration::{
        ArtifactRef, ArtifactRevision, TaskDefinition, TaskDefinitionRef, TaskRef,
    },
    services::{task_draft_import::TaskDraftPublicationPlan, task_store::TaskContentSnapshot},
};
use sha2::{Digest, Sha256};

mod confirmation;
mod observation;
#[cfg(test)]
mod observation_tests;
#[cfg(test)]
mod tests;

type PublicationResult<T> = std::result::Result<T, SessionDraftPublicationError>;

/// One caller-owned attempt. No imported state, manual conversion or retry surface.
/// Read-only target comparisons never adopt progress or restore authority.
/// Its frozen metadata is not an assessment capability, authorization grant or saved Task.
pub struct SessionCatalogTaskDraftPublication {
    source: DurableAuthSource,
    workspace: SessionCatalogTaskContentWorkspace,
    plan: TaskDraftPublicationPlan,
    definition: TaskDefinition,
    fingerprint: [u8; 32],
    task: TaskRef,
    allocated: Vec<ArtifactRef>,
    confirmed: Vec<ArtifactRevision>,
    completed: Option<TaskContentSnapshot>,
    state: SessionDraftPublicationState,
}

impl DurableAuthSource {
    /// Pure local preparation. Exact trusted metadata is frozen before allocating targets;
    /// token shape and source/scope agreement do not authenticate the caller or publish anything.
    pub fn prepare_catalog_task_draft_publication(
        &self,
        token: &str,
        workspace: SessionCatalogTaskContentWorkspace,
        reference: &TaskDefinitionRef,
        plan: TaskDraftPublicationPlan,
    ) -> PublicationResult<SessionCatalogTaskDraftPublication> {
        let scope = workspace.publication_scope();
        if !valid_token(token) || scope.authority_id != self.authority_id {
            return Err(SessionDraftPublicationError::InvalidPreparation);
        }
        let definition = workspace
            .publication_definition(reference)
            .cloned()
            .ok_or(SessionDraftPublicationError::InvalidPreparation)?;
        let task = TaskRef {
            workspace: scope,
            task_id: uuid::Uuid::new_v4()
                .try_into()
                .map_err(|_| SessionDraftPublicationError::InvalidPreparation)?,
        };
        let mut allocated: Vec<ArtifactRef> = Vec::with_capacity(plan.items().len());
        for item in plan.items() {
            let target = ArtifactRef {
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
                other.artifact_id == target.artifact_id || other.revision_id == target.revision_id
            }) {
                return Err(SessionDraftPublicationError::InvalidPreparation);
            }
            allocated.push(target);
        }
        Ok(SessionCatalogTaskDraftPublication {
            source: self.clone(),
            workspace,
            plan,
            definition,
            fingerprint: fingerprint(token),
            task,
            confirmed: Vec::with_capacity(allocated.len()),
            allocated,
            completed: None,
            state: SessionDraftPublicationState::Ready,
        })
    }
}

impl SessionCatalogTaskDraftPublication {
    /// Frozen definition metadata only, not a provider, permission or publication receipt.
    pub fn definition(&self) -> &TaskDefinition {
        &self.definition
    }

    pub fn state(&self) -> &SessionDraftPublicationState {
        &self.state
    }

    pub fn task_ref(&self) -> TaskRef {
        self.task
    }

    pub fn allocated_artifacts(&self) -> &[ArtifactRef] {
        &self.allocated
    }

    pub fn confirmed_artifacts(&self) -> &[ArtifactRevision] {
        &self.confirmed
    }

    pub fn confirmed_task(&self) -> Option<&TaskContentSnapshot> {
        self.completed.as_ref()
    }

    /// At most one existing current-Session write. An unpolled future does nothing; any
    /// dispatched error or dropped advance preserves the exact terminally unconfirmed target.
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
            let expected = manifest.clone();
            let workspace = self.workspace.clone();
            let reference = self.definition.reference.clone();
            self.await_task(
                expected,
                source.create_session_catalog_content_task(
                    token,
                    &workspace,
                    self.task.task_id,
                    &reference,
                    manifest,
                ),
            )
            .await
        }
    }
}

fn fingerprint(token: &str) -> [u8; 32] {
    Sha256::new()
        .chain_update(b"cyanrex.catalog-task-draft-publication.session.v1\0")
        .chain_update(token.as_bytes())
        .finalize()
        .into()
}
