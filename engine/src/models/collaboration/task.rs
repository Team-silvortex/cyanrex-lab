//! Versioned task catalogue contracts. They describe rules, not stored work or authorization.
use serde::{Deserialize, Serialize};

use super::{ContractError, CoreSchemaVersion, QualifiedName, RevisionNumber};

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

/// Exact work metadata with an optional automatic-assessment binding, not execution scheduling.
/// Registering this metadata confers no permissions and does not install executable code.
/// Both binding fields must be supplied: two explicit nulls mean no automatic assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawTaskDefinition")]
pub struct TaskDefinition {
    pub schema_version: CoreSchemaVersion,
    pub reference: TaskDefinitionRef,
    pub title: String,
    pub summary: String,
    pub evidence_schema: Option<VersionedName>,
    pub assessment_policy: Option<VersionedName>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTaskDefinition {
    schema_version: CoreSchemaVersion,
    reference: TaskDefinitionRef,
    title: String,
    summary: String,
    // A plain Option silently treats an omitted field as None. These fields are required
    // even when explicitly null; no default or omission-on-serialization is permitted.
    #[serde(deserialize_with = "required_optional_pin")]
    evidence_schema: Option<VersionedName>,
    #[serde(deserialize_with = "required_optional_pin")]
    assessment_policy: Option<VersionedName>,
}

fn required_optional_pin<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<VersionedName>, D::Error> {
    Option::<VersionedName>::deserialize(deserializer)
}

impl TryFrom<RawTaskDefinition> for TaskDefinition {
    type Error = ContractError;

    fn try_from(value: RawTaskDefinition) -> Result<Self, Self::Error> {
        if value.evidence_schema.is_some() != value.assessment_policy.is_some() {
            return Err(ContractError("task assessment binding must be complete"));
        }
        Ok(Self {
            schema_version: value.schema_version,
            reference: value.reference,
            title: value.title,
            summary: value.summary,
            evidence_schema: value.evidence_schema,
            assessment_policy: value.assessment_policy,
        })
    }
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
