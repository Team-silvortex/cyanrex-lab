//! Immutable, explicitly composed task definitions and typed rule dispatch.
//! No database, discovery, dynamic loading, execution, authentication or implicit fallback.
use crate::models::collaboration::{
    AssessmentOutcome, TaskAssessment, TaskDefinition, TaskDefinitionRef, VersionedName,
};

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
    definitions: Vec<TaskDefinition>,
}

impl<P: TaskProvider> TaskCatalog<P> {
    pub fn new(provider: P) -> Result<Self, TaskCatalogError> {
        let package = provider.package();
        let definitions = provider.definitions();
        if definitions.is_empty() || definitions.len() > 256 {
            return Err(TaskCatalogError::InvalidCatalog(
                "expected 1 to 256 definitions",
            ));
        }
        for (index, definition) in definitions.iter().enumerate() {
            validate_definition(&package, definition)?;
            if definitions[..index]
                .iter()
                .any(|other| other.reference == definition.reference)
            {
                return Err(TaskCatalogError::InvalidCatalog("duplicate definition"));
            }
        }
        Ok(Self {
            provider,
            definitions,
        })
    }

    pub fn definitions(&self) -> &[TaskDefinition] {
        &self.definitions
    }

    pub fn get(&self, reference: &TaskDefinitionRef) -> Option<&TaskDefinition> {
        self.definitions
            .iter()
            .find(|definition| &definition.reference == reference)
    }

    pub fn assess(
        &self,
        reference: &TaskDefinitionRef,
        evidence: P::Evidence<'_>,
    ) -> Result<TaskAssessment, TaskCatalogError> {
        let definition = self
            .get(reference)
            .ok_or(TaskCatalogError::UnknownDefinition)?;
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
            policy: definition.assessment_policy.clone(),
            evidence_schema: definition.evidence_schema.clone(),
            outcome,
        })
    }
}

pub(crate) fn validate_definition(
    package: &VersionedName,
    definition: &TaskDefinition,
) -> Result<(), TaskCatalogError> {
    let namespace = format!("{}.", package.name);
    if definition.reference.package != *package
        || !definition.reference.name.as_str().starts_with(&namespace)
        || !definition
            .evidence_schema
            .name
            .as_str()
            .starts_with(&namespace)
        || !definition
            .assessment_policy
            .name
            .as_str()
            .starts_with(&namespace)
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
