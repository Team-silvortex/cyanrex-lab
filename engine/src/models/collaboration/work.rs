//! Stored work is independent of execution state and automatic rule results.
use super::{
    ArtifactRef, CoreSchemaVersion, PrincipalRef, RevisionNumber, TaskDefinition, TaskRef,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Acceptance is deliberately absent until authenticated, revision-bound Review is implemented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Draft,
    Ready,
    InProgress,
    Blocked,
    InReview,
    /// Ends the work item only; cancelling an execution or external resource is a separate command.
    Cancelled,
}

impl TaskStatus {
    pub fn can_transition_to(self, next: Self) -> bool {
        use TaskStatus::*;
        matches!(
            (self, next),
            (Draft, Ready | Cancelled)
                | (Ready, InProgress | Blocked | Cancelled)
                | (InProgress, Blocked | InReview | Cancelled)
                | (Blocked, Ready | InProgress | Cancelled)
                | (InReview, InProgress | Cancelled)
        )
    }
}

/// A stored definition is a snapshot, not a request to resolve the latest installed package.
/// Artifact refs pin coordinates; their existence/visibility still require an Artifact service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSnapshot {
    pub schema_version: CoreSchemaVersion,
    pub reference: TaskRef,
    pub owner: PrincipalRef,
    pub title: String,
    pub definition: Option<TaskDefinition>,
    pub input_refs: Vec<ArtifactRef>,
    pub status: TaskStatus,
    pub revision: RevisionNumber,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
