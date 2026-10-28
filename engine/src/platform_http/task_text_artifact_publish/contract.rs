//! Map-only wire types, not deserializers or authority changes to the shared stored model.
use crate::{
    models::collaboration::{
        ArtifactId, ArtifactRef, ArtifactRevisionId, CoreSchemaVersion, PrincipalRef,
        RevisionNumber, Sha256Digest, TaskContentManifest, TaskId, TaskRef, TaskSnapshot,
        TaskStatus, WorkspaceRef,
    },
    platform_http::{contract::MapOnly, errors::HttpError},
    services::{
        artifact_store::validate_artifact_metadata, task_content::validate_text_bytes,
        task_store::TaskContentSnapshot,
    },
};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Create {
    schema_version: CoreSchemaVersion,
    expected: MapOnly<Expected>,
    artifact_id: ArtifactId,
    revision_id: ArtifactRevisionId,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    task: MapOnly<Task>,
    manifest: TaskContentManifest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Task {
    schema_version: CoreSchemaVersion,
    reference: MapOnly<Reference>,
    owner: MapOnly<PrincipalRef>,
    title: String,
    // Required explicit null. Option would silently accept a missing field; Value would erase
    // nested duplicate keys. This command admits only a manual Task, not a supplied definition.
    definition: (),
    input_refs: Vec<MapOnly<Pin>>,
    status: String,
    revision: RevisionNumber,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    workspace: MapOnly<WorkspaceRef>,
    task_id: TaskId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    workspace: MapOnly<WorkspaceRef>,
    artifact_id: ArtifactId,
    revision_id: ArtifactRevisionId,
    sha256: Sha256Digest,
}

impl Task {
    fn into_snapshot(self) -> Result<TaskSnapshot, HttpError> {
        let status = match self.status.as_str() {
            "draft" => TaskStatus::Draft,
            "ready" => TaskStatus::Ready,
            "in_progress" => TaskStatus::InProgress,
            "blocked" => TaskStatus::Blocked,
            "in_review" => TaskStatus::InReview,
            "cancelled" => TaskStatus::Cancelled,
            _ => return Err(HttpError::invalid_request()),
        };
        let () = self.definition;
        Ok(TaskSnapshot {
            schema_version: self.schema_version,
            reference: TaskRef {
                workspace: self.reference.0.workspace.0,
                task_id: self.reference.0.task_id,
            },
            owner: self.owner.0,
            title: self.title,
            definition: None,
            input_refs: self
                .input_refs
                .into_iter()
                .map(|pin| ArtifactRef {
                    workspace: pin.0.workspace.0,
                    artifact_id: pin.0.artifact_id,
                    revision_id: pin.0.revision_id,
                    sha256: pin.0.sha256,
                })
                .collect(),
            status,
            revision: self.revision,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

pub(super) struct Command {
    pub expected: TaskContentSnapshot,
    pub artifact: ArtifactId,
    pub revision: ArtifactRevisionId,
    pub text: String,
}

impl Create {
    pub(super) fn into_command(
        self,
        id: TaskId,
        scope: WorkspaceRef,
    ) -> Result<Command, HttpError> {
        let _ = self.schema_version;
        let expected = TaskContentSnapshot {
            task: self.expected.0.task.0.into_snapshot()?,
            manifest: self.expected.0.manifest,
        };
        if expected.task.reference.task_id != id || expected.task.reference.workspace != scope {
            return Err(HttpError::invalid_request());
        }
        expected
            .manifest
            .validate_task_snapshot(&expected.task)
            .map_err(|_| HttpError::invalid_request())?;
        validate_text_bytes(self.text.as_bytes()).map_err(|_| HttpError::invalid_request())?;
        validate_artifact_metadata(&expected.task.title, "text/plain", self.text.len() as u64)
            .map_err(|_| HttpError::invalid_request())?;
        Ok(Command {
            expected,
            artifact: self.artifact_id,
            revision: self.revision_id,
            text: self.text,
        })
    }
}
