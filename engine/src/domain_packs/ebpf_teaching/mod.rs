//! The teaching pack owns lab IDs, templates, source evidence and runtime assessment semantics.
use crate::{
    models::collaboration::{
        AssessmentOutcome, CoreSchemaVersion, TaskDefinition, TaskDefinitionRef, VersionedName,
    },
    services::task_catalog::{TaskCatalogError, TaskProvider},
};

pub mod legacy;
mod rules;
mod source;

use rules::{LabRule, LAB_RULES};

pub struct EbpfTeachingPack;

/// An observation from the existing trusted execution path, not a browser approval payload.
/// This provider evaluates observations; it never compiles, loads or attaches a program.
/// A different task type's evidence cannot enter this evaluator:
///
/// ```compile_fail
/// use cyanrex_engine::{
///     domain_packs::ebpf_teaching::EbpfTeachingPack,
///     services::task_catalog::TaskCatalog,
/// };
/// let catalog = TaskCatalog::new(EbpfTeachingPack).unwrap();
/// let reference = EbpfTeachingPack::definition_ref("01-first-program").unwrap();
/// catalog.assess(&reference, "a document is not a runtime observation");
/// ```
pub struct LabRunEvidence<'a> {
    pub template_id: Option<&'a str>,
    pub source: &'a str,
    pub run_success: bool,
    pub stage: &'a str,
    pub attach_expected: bool,
    pub attach_verified: bool,
}

impl EbpfTeachingPack {
    /// Legacy IDs are mapped explicitly, never parsed as arbitrary generic task names.
    pub fn definition_ref(lab_id: &str) -> Option<TaskDefinitionRef> {
        LAB_RULES
            .iter()
            .find(|rule| rule.id == lab_id)
            .map(task_ref)
    }
}

impl TaskProvider for EbpfTeachingPack {
    type Evidence<'a> = LabRunEvidence<'a>;

    fn package(&self) -> VersionedName {
        versioned("cyanrex.ebpf_teaching")
    }

    fn definitions(&self) -> Vec<TaskDefinition> {
        LAB_RULES.iter().map(definition).collect()
    }

    fn assess(
        &self,
        registered: &TaskDefinition,
        evidence: LabRunEvidence<'_>,
    ) -> Result<AssessmentOutcome, TaskCatalogError> {
        // Fail closed even if trusted Rust code calls the provider directly, bypassing the catalogue.
        let rule = LAB_RULES
            .iter()
            .find(|rule| definition(rule) == *registered)
            .ok_or(TaskCatalogError::UnknownDefinition)?;
        Ok(rules::assess(rule, evidence))
    }
}

fn task_ref(rule: &LabRule) -> TaskDefinitionRef {
    TaskDefinitionRef {
        package: versioned("cyanrex.ebpf_teaching"),
        name: format!("cyanrex.ebpf_teaching.lab-{}", rule.id)
            .parse()
            .expect("built-in task name"),
        version: 1.try_into().expect("built-in definition version"),
    }
}

fn definition(rule: &LabRule) -> TaskDefinition {
    TaskDefinition {
        schema_version: CoreSchemaVersion,
        reference: task_ref(rule),
        title: rule.title.to_owned(),
        summary: rule.summary.to_owned(),
        evidence_schema: versioned("cyanrex.ebpf_teaching.run_observation"),
        assessment_policy: versioned(&format!("cyanrex.ebpf_teaching.assess-{}", rule.id)),
    }
}

fn versioned(name: &str) -> VersionedName {
    VersionedName {
        name: name.parse().expect("built-in versioned name"),
        version: 1.try_into().expect("built-in contract version"),
    }
}
