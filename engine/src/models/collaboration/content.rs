//! Pure metadata for task-owned text content, not a browser draft or a saving command.
//!
//! Exact Artifact pins, filenames and language hints do not establish ownership, byte validity,
//! filesystem access, installed tooling or execution permission. Persistence and current-Session
//! authorization require a separately composed service boundary.

use super::{
    ArtifactId, ArtifactRef, ArtifactRevisionId, ContractError, CoreSchemaVersion, Sha256Digest,
    TaskSnapshot, WorkspaceRef,
};
use serde::{
    de::{value::MapAccessDeserializer, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use std::{fmt, marker::PhantomData};

pub const MAX_TASK_CONTENT_ITEMS: usize = 32;
pub const MAX_TASK_CONTENT_MANIFEST_BYTES: usize = 64 * 1024;
/// The separate byte-validation boundary enforces this; manifests contain no inline content.
pub const MAX_TEXT_PAYLOAD_BYTES: usize = 256 * 1024;

/// Serde structs also accept positional sequences; this contract admits only named records.
/// Forward the map directly so duplicate fields remain visible to each strict Raw deserializer.
struct MapOnly<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for MapOnly<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RecordVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for RecordVisitor<T> {
            type Value = MapOnly<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object record")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(MapOnly)
            }
        }

        deserializer.deserialize_map(RecordVisitor(PhantomData))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum TextPayloadKind {
    Text,
}

/// An exact text-content pin and display metadata, never a local editor identity or path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MapOnly<RawTextPayloadBinding>")]
pub struct TextPayloadBinding {
    kind: TextPayloadKind,
    filename: String,
    language: String,
    artifact: ArtifactRef,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTextPayloadBinding {
    kind: String,
    filename: String,
    language: String,
    artifact: MapOnly<RawArtifactRef>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArtifactRef {
    workspace: MapOnly<WorkspaceRef>,
    artifact_id: ArtifactId,
    revision_id: ArtifactRevisionId,
    sha256: Sha256Digest,
}

impl TryFrom<MapOnly<RawTextPayloadBinding>> for TextPayloadBinding {
    type Error = ContractError;

    fn try_from(value: MapOnly<RawTextPayloadBinding>) -> Result<Self, Self::Error> {
        let value = value.0;
        if value.kind != "text" {
            return Err(ContractError("unsupported task content payload kind"));
        }
        let artifact = value.artifact.0;
        Self::new(
            ArtifactRef {
                workspace: artifact.workspace.0,
                artifact_id: artifact.artifact_id,
                revision_id: artifact.revision_id,
                sha256: artifact.sha256,
            },
            value.filename,
            value.language,
        )
    }
}

impl TextPayloadBinding {
    pub fn new(
        artifact: ArtifactRef,
        filename: impl Into<String>,
        language: impl Into<String>,
    ) -> Result<Self, ContractError> {
        let filename = filename.into();
        let language = language.into();
        validate_text_payload_metadata(&filename, &language)?;
        Ok(Self {
            kind: TextPayloadKind::Text,
            filename,
            language,
            artifact,
        })
    }

    pub fn artifact(&self) -> &ArtifactRef {
        &self.artifact
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Unknown hints remain data; they do not select a provider, parser or executable.
    pub fn language(&self) -> &str {
        &self.language
    }
}

/// Shared inert-label rules for server manifests and pure portable-draft planning.
pub(crate) fn validate_text_payload_metadata(
    filename: &str,
    language: &str,
) -> Result<(), ContractError> {
    if filename.is_empty()
        // Match the local editor's UTF-16 bound without normalizing a display label.
        || filename.encode_utf16().count() > 128
        || matches!(filename, "." | "..")
        || filename.contains(['/', '\\'])
        || filename.chars().any(char::is_control)
    {
        return Err(ContractError(
            "text payload filename must be a bounded display basename",
        ));
    }
    if language.len() > 64
        || !language
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_lowercase)
        || !language.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'+' | b'.' | b'-')
        })
    {
        return Err(ContractError(
            "text payload language must be a bounded lowercase hint",
        ));
    }
    Ok(())
}

pub(crate) fn validate_task_content_title(title: &str) -> Result<(), ContractError> {
    if title.trim().is_empty() || title.len() > 256 || title.chars().any(char::is_control) {
        return Err(ContractError(
            "task content title must be nonblank bounded text",
        ));
    }
    Ok(())
}

/// Validated metadata that can describe a Task snapshot, but does not save or authorize it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MapOnly<RawTaskContentManifest>")]
pub struct TaskContentManifest {
    schema_version: CoreSchemaVersion,
    title: String,
    payload: Vec<TextPayloadBinding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTaskContentManifest {
    schema_version: CoreSchemaVersion,
    title: String,
    payload: Vec<TextPayloadBinding>,
}

impl TryFrom<MapOnly<RawTaskContentManifest>> for TaskContentManifest {
    type Error = ContractError;

    fn try_from(value: MapOnly<RawTaskContentManifest>) -> Result<Self, Self::Error> {
        let value = value.0;
        // Deserializing the scalar rejects unknown contract versions before construction.
        let CoreSchemaVersion = value.schema_version;
        Self::new(value.title, value.payload)
    }
}

impl TaskContentManifest {
    pub fn new(
        title: impl Into<String>,
        payload: Vec<TextPayloadBinding>,
    ) -> Result<Self, ContractError> {
        let title = title.into();
        validate_task_content_title(&title)?;
        if payload.len() > MAX_TASK_CONTENT_ITEMS {
            return Err(ContractError("task content has too many payload items"));
        }
        for (index, item) in payload.iter().enumerate() {
            if item.artifact.workspace != payload[0].artifact.workspace {
                return Err(ContractError("task content must use one workspace"));
            }
            if payload[..index].iter().any(|other| {
                other.artifact.workspace == item.artifact.workspace
                    && other.artifact.artifact_id == item.artifact.artifact_id
                    && other.artifact.revision_id == item.artifact.revision_id
            }) {
                return Err(ContractError(
                    "task content cannot repeat an artifact revision",
                ));
            }
        }
        Ok(Self {
            schema_version: CoreSchemaVersion,
            title,
            payload,
        })
    }

    /// Strict, bounded server metadata JSON; deliberately not the portable local-draft importer.
    /// Other Serde entry points validate structure but their caller must impose a byte budget.
    pub fn parse_json(bytes: &[u8]) -> Result<Self, ContractError> {
        if bytes.len() > MAX_TASK_CONTENT_MANIFEST_BYTES {
            return Err(ContractError(
                "task content manifest exceeds its byte limit",
            ));
        }
        serde_json::from_slice(bytes).map_err(|_| ContractError("invalid task content manifest"))
    }

    pub fn schema_version(&self) -> CoreSchemaVersion {
        self.schema_version
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn payload(&self) -> &[TextPayloadBinding] {
        &self.payload
    }

    pub fn input_refs(&self) -> Vec<ArtifactRef> {
        self.payload
            .iter()
            .map(|item| item.artifact.clone())
            .collect()
    }

    /// Structural read-only matching, not authentication, ownership or current-revision approval.
    /// Historical snapshots in any status may match; Artifact existence and bytes remain unchecked.
    pub fn validate_task_snapshot(&self, task: &TaskSnapshot) -> Result<(), ContractError> {
        if self.title != task.title
            || task.owner.authority_id != task.reference.workspace.authority_id
            || self.payload.len() != task.input_refs.len()
            || self
                .payload
                .iter()
                .zip(&task.input_refs)
                .any(|(item, input)| {
                    item.artifact != *input || item.artifact.workspace != task.reference.workspace
                })
        {
            return Err(ContractError(
                "task content manifest does not match the task snapshot",
            ));
        }
        Ok(())
    }
}
