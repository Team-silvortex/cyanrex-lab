//! Versioned task catalogue contracts. They describe rules, not stored work or authorization.
use serde::{Deserialize, Serialize};

use super::{CoreSchemaVersion, QualifiedName, RevisionNumber};

/// A package-local contract version, independent of the application's release number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionedName {
    pub name: QualifiedName,
    pub version: RevisionNumber,
}

/// An exact definition; there is deliberately no floating "latest" selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskDefinitionRef {
    pub package: VersionedName,
    pub name: QualifiedName,
    pub version: RevisionNumber,
}

/// The first definition slice covers rule assessment only, not workflow/execution scheduling.
/// Registering this metadata confers no permissions and does not install executable code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskDefinition {
    pub schema_version: CoreSchemaVersion,
    pub reference: TaskDefinitionRef,
    pub title: String,
    pub summary: String,
    pub evidence_schema: VersionedName,
    pub assessment_policy: VersionedName,
}

/// Rule success is not a human approval or a Task state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentVerdict {
    Passed,
    NotPassed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssessmentOutcome {
    pub verdict: AssessmentVerdict,
    pub feedback: Vec<String>,
}

/// An in-process rule result. This is not a durable Review: it lacks target revision,
/// actor and provenance, and must not be accepted from a client as an approval receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskAssessment {
    pub definition: TaskDefinitionRef,
    pub policy: VersionedName,
    pub evidence_schema: VersionedName,
    pub outcome: AssessmentOutcome,
}
