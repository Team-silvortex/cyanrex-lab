//! Exact catalogue admission; no provider is retained or executed inside authorization.
use super::*;
use crate::{
    models::collaboration::{TaskDefinition, TaskDefinitionRef},
    services::task_catalog::{TaskCatalog, TaskProvider},
};
use std::sync::Arc;

/// Trusted, frozen definition allowlist plus resource routing. Not a request or dynamic registry.
/// Clones share metadata, not provider code, runtime evidence or a detached permission grant.
#[derive(Clone)]
pub struct SessionCatalogTaskWorkspace {
    inputs: SessionTaskInputsWorkspace,
    definitions: Arc<[TaskDefinition]>,
}
impl SessionCatalogTaskWorkspace {
    /// Copies immutable catalogue metadata only; the provider need not be Send, Sync or retained.
    pub fn new<P: TaskProvider>(
        tasks: SessionTaskWorkspace,
        artifacts: SessionArtifactWorkspace,
        catalog: &TaskCatalog<P>,
    ) -> TaskResult<Self> {
        let inputs = SessionTaskInputsWorkspace::new(tasks, artifacts)?;
        Ok(Self {
            inputs,
            definitions: catalog.definitions().to_vec().into(),
        })
    }

    fn definition(&self, reference: &TaskDefinitionRef) -> TaskResult<&TaskDefinition> {
        self.definitions
            .iter()
            .find(|definition| &definition.reference == reference)
            .ok_or_else(|| TaskStoreError::UnknownDefinition.into())
    }

    pub(super) fn require_task(&self, task: &TaskSnapshot) -> TaskResult<()> {
        let stored = task
            .definition
            .as_ref()
            .ok_or(SessionTaskError::UnsupportedTask)?;
        if stored != self.definition(&stored.reference)? {
            return Err(SessionTaskError::DefinitionMismatch);
        }
        Ok(())
    }
}

impl DurableAuthSource {
    /// Replace only this owner's Draft inputs while preserving its exact admitted definition.
    /// Empty input lists are allowed; neither old nor new content validation may be skipped.
    pub async fn replace_session_catalog_task_inputs(
        &self,
        token: &str,
        workspace: &SessionCatalogTaskWorkspace,
        id: TaskId,
        revision: RevisionNumber,
        inputs: Vec<ArtifactRef>,
    ) -> TaskResult<TaskSnapshot> {
        replacement::validate_replacement_inputs(&workspace.inputs, &inputs, false)?;
        match self
            .session_input_task(
                token,
                &workspace.inputs,
                id,
                InputOperation::ReplaceInputs(revision, inputs),
                InputPolicy::Catalog(workspace),
            )
            .await?
        {
            InputPending::Task(task) => Ok(task),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    /// Current private membership plus exact definition admission, not evidence-schema validation.
    /// A definition does not grant execution, sharing, rule Review issuance or acceptance rights.
    pub async fn create_session_catalog_task(
        &self,
        token: &str,
        workspace: &SessionCatalogTaskWorkspace,
        id: TaskId,
        reference: &TaskDefinitionRef,
        title: &str,
        inputs: Vec<ArtifactRef>,
    ) -> TaskResult<TaskSnapshot> {
        if inputs.len() > 32 {
            return Err(TaskStoreError::InvalidInput.into());
        }
        let definition = workspace.definition(reference)?.clone();
        let draft = TaskDraft::from_definition(definition, title, inputs.clone())?;
        match self
            .session_input_task(
                token,
                &workspace.inputs,
                id,
                InputOperation::Create(draft, inputs),
                InputPolicy::Catalog(workspace),
            )
            .await?
        {
            InputPending::Task(task) => Ok(task),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    /// Full saved definition must still match this handle's allowlist. Never substitute latest.
    pub async fn get_session_catalog_task(
        &self,
        token: &str,
        workspace: &SessionCatalogTaskWorkspace,
        id: TaskId,
    ) -> TaskResult<Option<SessionTaskInputs>> {
        match self
            .session_input_task(
                token,
                &workspace.inputs,
                id,
                InputOperation::Read,
                InputPolicy::Catalog(workspace),
            )
            .await?
        {
            InputPending::Read(result) => Ok(result),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    /// The existing graph is unchanged; even cancellation requires admitted metadata and inputs.
    pub async fn transition_session_catalog_task(
        &self,
        token: &str,
        workspace: &SessionCatalogTaskWorkspace,
        id: TaskId,
        revision: RevisionNumber,
        status: TaskStatus,
    ) -> TaskResult<TaskSnapshot> {
        match self
            .session_input_task(
                token,
                &workspace.inputs,
                id,
                InputOperation::Transition(revision, status),
                InputPolicy::Catalog(workspace),
            )
            .await?
        {
            InputPending::Task(task) => Ok(task),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }
}
