use super::*;
use crate::services::task_catalog::{validate_definition, TaskCatalog, TaskProvider};

/// Only constructors admit drafts; deserialization cannot bypass catalogue lookup.
pub struct TaskDraft {
    pub(super) title: String,
    pub(super) definition: Option<TaskDefinition>,
    pub(super) inputs: Vec<ArtifactRef>,
}

impl TaskDraft {
    pub fn manual(title: impl Into<String>, inputs: Vec<ArtifactRef>) -> Result<Self> {
        let draft = Self {
            title: title.into(),
            definition: None,
            inputs,
        };
        validate_content(&draft.title, &draft.definition, &draft.inputs)?;
        Ok(draft)
    }

    pub fn from_catalog<P: TaskProvider>(
        catalog: &TaskCatalog<P>,
        reference: &TaskDefinitionRef,
        title: impl Into<String>,
        inputs: Vec<ArtifactRef>,
    ) -> Result<Self> {
        let definition = catalog
            .get(reference)
            .ok_or(TaskStoreError::UnknownDefinition)?
            .clone();
        Self::from_definition(definition, title, inputs)
    }

    /// Trusted frozen metadata selected by a crate-internal adapter, not a public lookup bypass.
    pub(crate) fn from_definition(
        definition: TaskDefinition,
        title: impl Into<String>,
        inputs: Vec<ArtifactRef>,
    ) -> Result<Self> {
        let draft = Self {
            title: title.into(),
            definition: Some(definition),
            inputs,
        };
        validate_content(&draft.title, &draft.definition, &draft.inputs)?;
        Ok(draft)
    }
}

pub(super) fn validate_content(
    title: &str,
    definition: &Option<TaskDefinition>,
    inputs: &[ArtifactRef],
) -> Result<()> {
    if title.trim().is_empty() || title.len() > 256 || title.chars().any(char::is_control) {
        return Err(TaskStoreError::InvalidInput);
    }
    validate_input_refs(inputs)?;
    if let Some(definition) = definition {
        validate_definition(&definition.reference.package, definition)
            .map_err(|_| TaskStoreError::InvalidInput)?;
    }
    Ok(())
}

/// Structural admission only; scope, ownership and exact content need the authorized adapter.
pub(crate) fn validate_input_refs(inputs: &[ArtifactRef]) -> Result<()> {
    if inputs.len() > 32 {
        return Err(TaskStoreError::InvalidInput);
    }
    for (index, input) in inputs.iter().enumerate() {
        // Same revision cannot appear twice, even with a conflicting digest.
        if inputs[..index].iter().any(|other| {
            other.workspace == input.workspace
                && other.artifact_id == input.artifact_id
                && other.revision_id == input.revision_id
        }) {
            return Err(TaskStoreError::InvalidInput);
        }
    }
    Ok(())
}
