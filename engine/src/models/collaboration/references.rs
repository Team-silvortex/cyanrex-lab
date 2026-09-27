use serde::{Deserialize, Serialize};

use super::{
    ArtifactId, ArtifactRevisionId, AuthorityId, PrincipalId, ReviewId, RunId, Sha256Digest,
    TaskId, WorkspaceId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalRef {
    pub authority_id: AuthorityId,
    pub principal_id: PrincipalId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRef {
    pub authority_id: AuthorityId,
    pub workspace_id: WorkspaceId,
}

/// A pinned revision, never a floating "latest" link or a network-fetch instruction.
/// Equal blobs do not imply equal ownership or permission to read either artifact.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRef {
    pub workspace: WorkspaceRef,
    pub artifact_id: ArtifactId,
    pub revision_id: ArtifactRevisionId,
    pub sha256: Sha256Digest,
}

/// The containing envelope supplies the authority and workspace for this aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AggregateRef {
    Artifact(ArtifactId),
    Task(TaskId),
    Run(RunId),
    Review(ReviewId),
    Workspace(WorkspaceId),
}
