//! Frozen definition metadata, independent from typed evidence and executable assessment.
use super::{
    validate_definition, TaskCatalogError, TaskDefinition, TaskDefinitionRef, VersionedName,
};
use std::sync::Arc;

/// Trusted explicit registration only, never request decoding, package discovery or authority.
#[derive(Clone)]
pub struct TaskDefinitionCatalog {
    definitions: Arc<[TaskDefinition]>,
}

impl TaskDefinitionCatalog {
    pub fn new(
        package: VersionedName,
        definitions: Vec<TaskDefinition>,
    ) -> Result<Self, TaskCatalogError> {
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
            definitions: definitions.into(),
        })
    }

    /// Explicitly combines frozen metadata, never providers or execution capabilities.
    /// The flattened definition limit is checked before copying. Duplicate exact references
    /// always fail, even when their metadata agrees; source order does not confer precedence.
    pub fn compose(catalogs: &[Self]) -> Result<Self, TaskCatalogError> {
        if catalogs.is_empty() || catalogs.len() > 256 {
            return Err(TaskCatalogError::InvalidCatalog(
                "expected 1 to 256 catalogs",
            ));
        }
        let count = catalogs.iter().try_fold(0usize, |total, catalog| {
            total
                .checked_add(catalog.definitions.len())
                .filter(|count| *count <= 256)
                .ok_or(TaskCatalogError::InvalidCatalog(
                    "expected 1 to 256 definitions",
                ))
        })?;
        if count == 0 {
            return Err(TaskCatalogError::InvalidCatalog(
                "expected 1 to 256 definitions",
            ));
        }
        let mut definitions: Vec<TaskDefinition> = Vec::with_capacity(count);
        for catalog in catalogs {
            for definition in catalog.definitions.iter() {
                validate_definition(&definition.reference.package, definition)?;
                if definitions
                    .iter()
                    .any(|other| other.reference == definition.reference)
                {
                    return Err(TaskCatalogError::InvalidCatalog("duplicate definition"));
                }
                definitions.push(definition.clone());
            }
        }
        Ok(Self {
            definitions: definitions.into(),
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
}
