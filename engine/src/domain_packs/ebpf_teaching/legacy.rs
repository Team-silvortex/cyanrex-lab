//! Existing teaching API projection. No record schema, authorization or completion rule changes.
use std::sync::OnceLock;

use crate::{
    models::{collaboration::AssessmentVerdict, learning::LabDefinition},
    services::task_catalog::TaskCatalog,
};

use super::{
    rules::{LabRule, LAB_RULES},
    task_ref, EbpfTeachingPack, LabRunEvidence,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabAssessment {
    pub completed: bool,
    pub feedback: Vec<String>,
}

fn catalog() -> &'static TaskCatalog<EbpfTeachingPack> {
    static CATALOG: OnceLock<TaskCatalog<EbpfTeachingPack>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        TaskCatalog::new(EbpfTeachingPack).expect("valid built-in teaching catalog")
    })
}

pub fn lab_definitions() -> Vec<LabDefinition> {
    LAB_RULES.iter().map(to_legacy_definition).collect()
}

pub fn find_lab(lab_id: &str) -> Option<LabDefinition> {
    LAB_RULES
        .iter()
        .find(|rule| rule.id == lab_id)
        .map(to_legacy_definition)
}

pub fn assess_lab_run(
    lab_id: &str,
    template_id: Option<&str>,
    source: &str,
    run_success: bool,
    stage: &str,
    attach_expected: bool,
    attach_verified: bool,
) -> Result<LabAssessment, String> {
    let reference = EbpfTeachingPack::definition_ref(lab_id)
        .ok_or_else(|| format!("unknown lab id: {lab_id}"))?;
    let assessment = catalog()
        .assess(
            &reference,
            LabRunEvidence {
                template_id,
                source,
                run_success,
                stage,
                attach_expected,
                attach_verified,
            },
        )
        .map_err(|error| error.to_string())?;
    Ok(LabAssessment {
        completed: assessment.outcome.verdict == AssessmentVerdict::Passed,
        feedback: assessment.outcome.feedback,
    })
}

fn to_legacy_definition(rule: &LabRule) -> LabDefinition {
    let reference = task_ref(rule);
    let definition = catalog().get(&reference).expect("registered built-in lab");
    LabDefinition {
        id: rule.id.to_string(),
        position: rule.position,
        title: definition.title.clone(),
        summary: definition.summary.clone(),
        doc_slug: format!("labs/{}", rule.id),
        template_id: rule.template_id.map(str::to_string),
    }
}
