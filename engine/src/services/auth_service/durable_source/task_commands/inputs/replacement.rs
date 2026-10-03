//! Replace exact input pins, never publish files or infer a domain result from edited text.
use super::*;
use crate::services::task_store::validate_input_refs;

pub(super) fn validate_replacement_inputs(
    workspace: &SessionTaskInputsWorkspace,
    inputs: &[ArtifactRef],
    manual: bool,
) -> TaskResult<()> {
    if manual && inputs.is_empty() {
        return Err(SessionTaskError::UnsupportedTask);
    }
    validate_input_refs(inputs)?;
    if inputs
        .iter()
        .any(|input| input.workspace != workspace.tasks.scope)
    {
        return Err(TaskStoreError::ScopeMismatch.into());
    }
    Ok(())
}

pub(super) fn replacement_references(
    old: &TaskSnapshot,
    revision: RevisionNumber,
    inputs: &[ArtifactRef],
    policy: &InputPolicy<'_>,
) -> TaskResult<Vec<ArtifactRef>> {
    if old.revision != revision {
        return Err(TaskStoreError::StaleRevision.into());
    }
    if old.status != TaskStatus::Draft {
        return Err(TaskStoreError::InvalidTransition.into());
    }
    if old.input_refs == inputs {
        return Err(TaskStoreError::InvalidInput.into());
    }
    let mut proposed = old.clone();
    proposed.input_refs = inputs.to_vec();
    policy.require_task(&proposed)?;
    // Both lists are bounded to 32. Verify their entire union in one global Artifact lock order.
    // Keep duplicate coordinates: an old valid digest must not hide a new, forged digest.
    let mut references = old.input_refs.clone();
    references.extend_from_slice(inputs);
    Ok(references)
}

impl DurableAuthSource {
    /// Current owner's Draft only. Files must already exist as exact immutable Artifact revisions.
    /// No-op/replay is not success, and failure never permits deleting the published new content.
    pub async fn replace_session_input_task_inputs(
        &self,
        token: &str,
        workspace: &SessionTaskInputsWorkspace,
        id: TaskId,
        revision: RevisionNumber,
        inputs: Vec<ArtifactRef>,
    ) -> TaskResult<TaskSnapshot> {
        validate_replacement_inputs(workspace, &inputs, true)?;
        match self
            .session_input_task(
                token,
                workspace,
                id,
                InputOperation::ReplaceInputs(revision, inputs),
                InputPolicy::Manual,
            )
            .await?
        {
            InputPending::Task(task) => Ok(task),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }
}
