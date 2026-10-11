//! Immutable, explicitly composed task definitions and typed rule dispatch.
//! No database, discovery, dynamic loading, execution, authentication or implicit fallback.
use crate::models::collaboration::{
    AssessmentOutcome, TaskAssessment, TaskDefinition, TaskDefinitionRef, VersionedName,
};

mod definitions;
pub use definitions::TaskDefinitionCatalog;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TaskCatalogError {
    #[error("invalid task catalog: {0}")]
    InvalidCatalog(&'static str),
    #[error("unsupported task definition")]
    UnknownDefinition,
    #[error("invalid task evidence: {0}")]
    InvalidEvidence(&'static str),
    #[error("invalid task assessment")]
    InvalidAssessment,
}

/// Trusted built-in code only. Evidence is provider-specific and cannot be substituted with
/// an unrelated provider's payload. Implementations own evidence validation and resource limits;
/// the catalogue validates metadata, exact lookup and result size, not evidence authenticity.
/// Rules must remain fixed for each published definition/policy version.
pub trait TaskProvider {
    type Evidence<'a>;

    fn package(&self) -> VersionedName;
    fn definitions(&self) -> Vec<TaskDefinition>;
    fn assess(
        &self,
        definition: &TaskDefinition,
        evidence: Self::Evidence<'_>,
    ) -> Result<AssessmentOutcome, TaskCatalogError>;
}

pub struct TaskCatalog<P: TaskProvider> {
    provider: P,
    definitions: TaskDefinitionCatalog,
}

impl<P: TaskProvider> TaskCatalog<P> {
    pub fn new(provider: P) -> Result<Self, TaskCatalogError> {
        let package = provider.package();
        let definitions = TaskDefinitionCatalog::new(package, provider.definitions())?;
        if definitions.definitions().iter().any(|definition| {
            definition.evidence_schema.is_none() || definition.assessment_policy.is_none()
        }) {
            return Err(TaskCatalogError::InvalidCatalog(
                "typed providers require assessment bindings",
            ));
        }
        Ok(Self {
            provider,
            definitions,
        })
    }

    pub fn definitions(&self) -> &[TaskDefinition] {
        self.definitions.definitions()
    }

    /// Frozen metadata only; cloning this handle retains no provider or evidence capability.
    pub fn definition_catalog(&self) -> &TaskDefinitionCatalog {
        &self.definitions
    }

    pub fn get(&self, reference: &TaskDefinitionRef) -> Option<&TaskDefinition> {
        self.definitions.get(reference)
    }

    pub fn assess(
        &self,
        reference: &TaskDefinitionRef,
        evidence: P::Evidence<'_>,
    ) -> Result<TaskAssessment, TaskCatalogError> {
        let definition = self
            .get(reference)
            .ok_or(TaskCatalogError::UnknownDefinition)?;
        let evidence_schema =
            definition
                .evidence_schema
                .as_ref()
                .ok_or(TaskCatalogError::InvalidCatalog(
                    "typed providers require assessment bindings",
                ))?;
        let assessment_policy =
            definition
                .assessment_policy
                .as_ref()
                .ok_or(TaskCatalogError::InvalidCatalog(
                    "typed providers require assessment bindings",
                ))?;
        let outcome = self.provider.assess(definition, evidence)?;
        if outcome.feedback.len() > 64
            || outcome.feedback.iter().any(|message| message.len() > 2048)
            || outcome.feedback.iter().map(String::len).sum::<usize>() > 16_384
        {
            return Err(TaskCatalogError::InvalidAssessment);
        }
        // Identity is taken from the immutable registration, never supplied by an evaluator result.
        Ok(TaskAssessment {
            definition: definition.reference.clone(),
            policy: assessment_policy.clone(),
            evidence_schema: evidence_schema.clone(),
            outcome,
        })
    }
}

pub(crate) fn validate_definition(
    package: &VersionedName,
    definition: &TaskDefinition,
) -> Result<(), TaskCatalogError> {
    let binding = match (&definition.evidence_schema, &definition.assessment_policy) {
        (Some(evidence_schema), Some(assessment_policy)) => {
            Some((evidence_schema, assessment_policy))
        }
        (None, None) => None,
        _ => {
            return Err(TaskCatalogError::InvalidCatalog(
                "task assessment binding must be complete",
            ))
        }
    };
    let namespace = format!("{}.", package.name);
    if definition.reference.package != *package
        || !definition.reference.name.as_str().starts_with(&namespace)
        || binding.is_some_and(|(evidence_schema, assessment_policy)| {
            !evidence_schema.name.as_str().starts_with(&namespace)
                || !assessment_policy.name.as_str().starts_with(&namespace)
        })
    {
        return Err(TaskCatalogError::InvalidCatalog(
            "definition belongs to another package",
        ));
    }
    if definition.title.trim().is_empty()
        || definition.title.len() > 256
        || definition.title.chars().any(char::is_control)
        || definition.summary.len() > 2048
    {
        return Err(TaskCatalogError::InvalidCatalog("invalid display metadata"));
    }
    Ok(())
}
