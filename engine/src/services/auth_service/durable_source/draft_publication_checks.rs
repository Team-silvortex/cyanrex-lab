//! Shared pure publication consistency checks, never storage access or authorization decisions.
use super::SessionDraftPublicationError;
use crate::{
    models::collaboration::{
        ArtifactContent, ArtifactRef, ArtifactRevision, TaskContentManifest, TaskRef,
    },
    services::task_draft_import::TaskDraftPublicationPlan,
};

pub(in crate::services::auth_service::durable_source) fn artifact_matches_planned_create(
    plan: &TaskDraftPublicationPlan,
    task: TaskRef,
    allocated: &[ArtifactRef],
    confirmed: &[ArtifactRevision],
    index: usize,
    revision: &ArtifactRevision,
) -> bool {
    let (Some(item), Some(target)) = (plan.items().get(index), allocated.get(index)) else {
        return false;
    };
    revision.reference == *target
        && revision.owner.authority_id == task.workspace.authority_id
        && confirmed
            .first()
            .is_none_or(|first| first.owner == revision.owner)
        && u64::from(revision.sequence) == 1
        && revision.parent.is_none()
        && revision.kind.as_str() == "cyanrex.task-text"
        && revision.title == plan.title()
        && revision.media_type == "text/plain"
        && revision.byte_length == item.text().len() as u64
}

pub(in crate::services::auth_service::durable_source) fn publication_manifest(
    plan: &TaskDraftPublicationPlan,
    task: TaskRef,
    confirmed: &[ArtifactRevision],
) -> Result<TaskContentManifest, SessionDraftPublicationError> {
    // zip must not hide an inconsistent excess prefix, even in internally constructed state.
    if confirmed.len() != plan.items().len() {
        return Err(SessionDraftPublicationError::InvalidConfirmation);
    }
    if confirmed.is_empty() {
        // The eventual Task command derives its actual owner. Do not invent an empty-plan owner.
        return TaskContentManifest::new(plan.title(), vec![])
            .map_err(|_| SessionDraftPublicationError::InvalidPreparation);
    }
    let owner = confirmed
        .first()
        .ok_or(SessionDraftPublicationError::InvalidConfirmation)?
        .owner;
    let contents: Vec<_> = confirmed
        .iter()
        .zip(plan.items())
        .map(|(revision, item)| ArtifactContent {
            revision: revision.clone(),
            bytes: item.text().as_bytes().to_vec(),
        })
        .collect();
    // Original planned bytes and confirmed metadata are consistency evidence only. The final
    // current-Session Task transaction must independently recheck persisted metadata and blobs.
    plan.bind_published(task.workspace, owner, &contents)
        .map_err(|_| SessionDraftPublicationError::InvalidConfirmation)
}
