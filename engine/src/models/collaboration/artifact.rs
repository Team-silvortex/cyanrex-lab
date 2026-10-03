//! Immutable content metadata, independent of a Task, Run, domain provider or approval.
use super::{ArtifactRef, CoreSchemaVersion, PrincipalRef, QualifiedName, RevisionNumber};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRevision {
    pub schema_version: CoreSchemaVersion,
    pub reference: ArtifactRef,
    pub owner: PrincipalRef,
    pub sequence: RevisionNumber,
    pub parent: Option<ArtifactRef>,
    pub kind: QualifiedName,
    pub title: String,
    pub media_type: String,
    pub byte_length: u64,
    pub created_at: DateTime<Utc>,
}

/// Bytes have been checked against this revision, but do not constitute review or execution evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactContent {
    pub revision: ArtifactRevision,
    pub bytes: Vec<u8>,
}
