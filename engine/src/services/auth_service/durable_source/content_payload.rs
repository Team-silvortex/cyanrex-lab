//! Shared exact old/new text checks, not authority, publication, a transaction or a commit.
use super::SessionTaskError;
use crate::{
    models::collaboration::{
        ArtifactContent, PrincipalRef, RevisionNumber, TaskContentManifest, TaskStatus,
        TextPayloadBinding,
    },
    services::{
        artifact_store::{ArtifactContentReader, ArtifactStoreError},
        task_content::validate_text_payload_content,
        task_store::{TaskContentSnapshot, TaskStoreError},
    },
    sqlx_compat::Postgres,
};
use sqlx_core::transaction::Transaction;

type TaskResult<T> = Result<T, SessionTaskError>;

pub(crate) fn replacement_content_checks(
    old: &TaskContentSnapshot,
    expected: RevisionNumber,
    manifest: &TaskContentManifest,
) -> TaskResult<Vec<(TextPayloadBinding, TaskStoreError)>> {
    if old.task.revision != expected {
        return Err(TaskStoreError::StaleRevision.into());
    }
    if old.task.status != TaskStatus::Draft {
        return Err(TaskStoreError::InvalidTransition.into());
    }
    if old.manifest == *manifest {
        return Err(TaskStoreError::InvalidInput.into());
    }
    Ok(old
        .manifest
        .payload()
        .iter()
        .cloned()
        .map(|item| (item, TaskStoreError::InvalidRecord))
        .chain(
            manifest
                .payload()
                .iter()
                .cloned()
                .map(|item| (item, TaskStoreError::InvalidInput)),
        )
        .collect())
}

/// Keep one global lock order across the full old/new union, including duplicate coordinates:
/// valid old bytes must not mask a new conflicting digest. Only reads retain body buffers.
pub(crate) async fn verify_pinned_contents(
    tx: &mut Transaction<'_, Postgres>,
    reader: &ArtifactContentReader,
    owner: PrincipalRef,
    checks: &[(TextPayloadBinding, TaskStoreError)],
    retain: bool,
) -> TaskResult<Vec<ArtifactContent>> {
    reader.validate_namespace_in_transaction(tx).await?;
    let mut ordered: Vec<_> = checks.iter().enumerate().collect();
    ordered.sort_by_key(|(_, (item, _))| {
        (
            item.artifact().artifact_id.as_uuid(),
            item.artifact().revision_id.as_uuid(),
        )
    });
    let mut contents = Vec::new();
    for (index, (item, invalid)) in ordered {
        let content = reader
            .read_in_transaction(tx, item.artifact(), owner)
            .await?
            .ok_or(ArtifactStoreError::NotFound)?;
        validate_text_payload_content(item, owner, &content).map_err(|_| *invalid)?;
        if retain {
            contents.push((index, content));
        }
    }
    contents.sort_by_key(|(index, _)| *index);
    Ok(contents.into_iter().map(|(_, content)| content).collect())
}

#[cfg(test)]
#[path = "content_payload_tests.rs"]
mod tests;
