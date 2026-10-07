//! Bounded metadata records only, never persisted intent, imported receipts or recovery authority.
use super::{
    SessionDraftPublicationState, SessionDraftPublicationStep, SessionTaskDraftPublication,
};
use crate::models::collaboration::{
    ContractError, TaskContentManifest, TaskId, TaskRef, TextPayloadBinding, WorkspaceRef,
    MAX_TEXT_PAYLOAD_BYTES,
};
use serde::{
    de::{value::MapAccessDeserializer, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use std::{fmt, marker::PhantomData};

pub const MAX_SESSION_DRAFT_CHECKPOINT_BYTES: usize = 64 * 1024;
const FORMAT: &str = "cyanrex.task-draft-checkpoint";
const INVALID: ContractError = ContractError("invalid draft publication checkpoint");

/// A report in an untrusted record, not the writable state of a live attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDraftCheckpointState {
    Ready,
    Unconfirmed,
    Complete,
}

impl SessionDraftCheckpointState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unconfirmed => "unconfirmed",
            Self::Complete => "complete",
        }
    }
}

/// Metadata may still be private. Intentionally no Debug, Clone or public Serde implementation.
/// Parsing establishes only structural agreement, not authentic acknowledgements, actual bytes,
/// original ownership, storage incarnation or permission to read, write, retry or restore an attempt.
pub struct SessionDraftPublicationCheckpoint {
    task: TaskRef,
    manifest: TaskContentManifest,
    byte_lengths: Vec<u64>,
    reported_state: SessionDraftCheckpointState,
    reported_confirmed_artifact_count: usize,
}

// Keep every named record map-only without erasing duplicate fields through serde_json::Value.
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCheckpoint {
    format: String,
    version: u8,
    task: MapOnly<RawTaskRef>,
    manifest: TaskContentManifest,
    byte_lengths: Vec<u64>,
    reported_state: String,
    reported_confirmed_artifact_count: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTaskRef {
    workspace: MapOnly<WorkspaceRef>,
    task_id: TaskId,
}

#[derive(Serialize)]
struct WireCheckpoint<'a> {
    format: &'static str,
    version: u8,
    task: TaskRef,
    manifest: &'a TaskContentManifest,
    byte_lengths: &'a [u64],
    reported_state: &'static str,
    reported_confirmed_artifact_count: usize,
}

impl SessionDraftPublicationCheckpoint {
    pub fn parse_json(bytes: &[u8]) -> Result<Self, ContractError> {
        if bytes.len() > MAX_SESSION_DRAFT_CHECKPOINT_BYTES {
            return Err(INVALID);
        }
        let MapOnly(raw): MapOnly<RawCheckpoint> =
            serde_json::from_slice(bytes).map_err(|_| INVALID)?;
        if raw.format != FORMAT || raw.version != 1 {
            return Err(INVALID);
        }
        let reported_state = match raw.reported_state.as_str() {
            "ready" => SessionDraftCheckpointState::Ready,
            "unconfirmed" => SessionDraftCheckpointState::Unconfirmed,
            "complete" => SessionDraftCheckpointState::Complete,
            _ => return Err(INVALID),
        };
        let checkpoint = Self {
            task: TaskRef {
                workspace: raw.task.0.workspace.0,
                task_id: raw.task.0.task_id,
            },
            manifest: raw.manifest,
            byte_lengths: raw.byte_lengths,
            reported_state,
            reported_confirmed_artifact_count: raw.reported_confirmed_artifact_count,
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    /// Produces bytes only. The caller owns any saving policy; no durable intent precedes writes.
    pub fn to_json(&self) -> Result<Vec<u8>, ContractError> {
        self.validate()?;
        let bytes = serde_json::to_vec(&WireCheckpoint {
            format: FORMAT,
            version: 1,
            task: self.task,
            manifest: &self.manifest,
            byte_lengths: &self.byte_lengths,
            reported_state: self.reported_state.as_str(),
            reported_confirmed_artifact_count: self.reported_confirmed_artifact_count,
        })
        .map_err(|_| INVALID)?;
        if bytes.len() > MAX_SESSION_DRAFT_CHECKPOINT_BYTES {
            return Err(INVALID);
        }
        Ok(bytes)
    }

    fn validate(&self) -> Result<(), ContractError> {
        let items = self.manifest.payload();
        let count = self.reported_confirmed_artifact_count;
        if !allocated_id(self.task.task_id.as_uuid())
            || count > items.len()
            || (self.reported_state == SessionDraftCheckpointState::Complete
                && count != items.len())
            || self.byte_lengths.len() != items.len()
            || self
                .byte_lengths
                .iter()
                .any(|length| *length > MAX_TEXT_PAYLOAD_BYTES as u64)
        {
            return Err(INVALID);
        }
        for (index, item) in items.iter().enumerate() {
            let reference = item.artifact();
            if reference.workspace != self.task.workspace
                || !allocated_id(reference.artifact_id.as_uuid())
                || !allocated_id(reference.revision_id.as_uuid())
                || items[..index].iter().any(|other| {
                    other.artifact().artifact_id == reference.artifact_id
                        || other.artifact().revision_id == reference.revision_id
                })
            {
                return Err(INVALID);
            }
        }
        Ok(())
    }

    pub fn task_ref(&self) -> TaskRef {
        self.task
    }

    pub fn manifest(&self) -> &TaskContentManifest {
        &self.manifest
    }

    pub fn byte_lengths(&self) -> &[u64] {
        &self.byte_lengths
    }

    pub fn reported_state(&self) -> SessionDraftCheckpointState {
        self.reported_state
    }

    pub fn reported_confirmed_artifact_count(&self) -> usize {
        self.reported_confirmed_artifact_count
    }

    /// Derived data only. Even Ready or Complete records may be stale or fabricated.
    pub fn reported_unconfirmed_step(&self) -> Option<SessionDraftPublicationStep> {
        if self.reported_state != SessionDraftCheckpointState::Unconfirmed {
            return None;
        }
        let index = self.reported_confirmed_artifact_count;
        Some(match self.manifest.payload().get(index) {
            Some(item) => SessionDraftPublicationStep::Artifact {
                index,
                reference: item.artifact().clone(),
            },
            None => SessionDraftPublicationStep::Task {
                reference: self.task,
            },
        })
    }
}

fn allocated_id(id: uuid::Uuid) -> bool {
    id.get_version() == Some(uuid::Version::Random) && id.get_variant() == uuid::Variant::RFC4122
}

impl SessionTaskDraftPublication {
    /// Exports a local report, not the live attempt, its credentials or a reusable receipt.
    /// No database/file access occurs, and exporting never changes confirmation or retry state.
    pub fn checkpoint(&self) -> Result<SessionDraftPublicationCheckpoint, ContractError> {
        if self.allocated.len() != self.plan.items().len()
            || self.confirmed.len() > self.allocated.len()
        {
            return Err(INVALID);
        }
        let payload = self
            .allocated
            .iter()
            .zip(self.plan.items())
            .map(|(reference, item)| {
                TextPayloadBinding::new(reference.clone(), item.filename(), item.language())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let checkpoint = SessionDraftPublicationCheckpoint {
            task: self.task,
            manifest: TaskContentManifest::new(self.plan.title(), payload)?,
            byte_lengths: self
                .plan
                .items()
                .iter()
                .map(|item| item.text().len() as u64)
                .collect(),
            reported_state: match self.state {
                SessionDraftPublicationState::Ready => SessionDraftCheckpointState::Ready,
                SessionDraftPublicationState::Unconfirmed(_) => {
                    SessionDraftCheckpointState::Unconfirmed
                }
                SessionDraftPublicationState::Complete => SessionDraftCheckpointState::Complete,
            },
            reported_confirmed_artifact_count: self.confirmed.len(),
        };
        checkpoint.validate()?;
        // Export only internally consistent live state. This cannot make parsed claims authentic.
        let state_matches = match &self.state {
            SessionDraftPublicationState::Ready => self.completed.is_none(),
            SessionDraftPublicationState::Unconfirmed(step) => {
                self.completed.is_none()
                    && checkpoint.reported_unconfirmed_step().as_ref() == Some(step)
            }
            SessionDraftPublicationState::Complete => self
                .completed
                .as_ref()
                .is_some_and(|task| self.task_matches_planned_create(&checkpoint.manifest, task)),
        };
        if !state_matches
            || self
                .confirmed
                .iter()
                .enumerate()
                .any(|(index, revision)| !self.artifact_matches_planned_create(index, revision))
        {
            return Err(INVALID);
        }
        // Use the same bounded wire contract even when the value originates from this process.
        checkpoint.to_json()?;
        Ok(checkpoint)
    }
}
