//! Revision-bound judgments, not authentication, verified evidence or Task acceptance.
use super::{
    ArtifactRef, CoreSchemaVersion, PrincipalRef, ReviewRef, TaskAssessment, VersionedName,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HumanReviewVerdict {
    Approved,
    ChangesRequested,
    Commented,
}

/// A rule result has no human verdict. Sources describe trusted adapter input, not verified
/// Principal kinds. Agent judgments are not accepted until their provenance contract exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReviewJudgment {
    Human {
        policy: VersionedName,
        verdict: HumanReviewVerdict,
        comment: String,
    },
    Rule {
        assessment: TaskAssessment,
    },
}

/// All target/evidence coordinates are pinned, never floating latest links. Reading this record
/// does not verify Artifact bytes, access, current reviewer authority or acceptance policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRecord {
    pub schema_version: CoreSchemaVersion,
    pub reference: ReviewRef,
    pub reviewer: PrincipalRef,
    pub targets: Vec<ArtifactRef>,
    pub evidence: Vec<ArtifactRef>,
    pub judgment: ReviewJudgment,
    pub supersedes: Option<ReviewRef>,
    pub created_at: DateTime<Utc>,
}
