use super::*;
use chrono::{DateTime, Utc};

impl TaskStore {
    /// Pending result only. The source-owned adapter must validate the old/new exact Artifact
    /// union and current Session on the same transaction, then confirm commit before publishing.
    pub(crate) async fn replace_inputs_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        reference: TaskRef,
        owner: PrincipalRef,
        expected_revision: RevisionNumber,
        inputs: Vec<ArtifactRef>,
    ) -> Result<TaskSnapshot> {
        self.check_scope(reference, owner)?;
        self.check_schema(tx, true).await?;
        let current = self
            .read(tx, reference, owner, true)
            .await?
            .ok_or(TaskStoreError::NotFound)?;
        let snapshot = prepare_replacement(&current, expected_revision, inputs, Utc::now())?;
        self.update_and_confirm(tx, &snapshot, expected_revision)
            .await?;
        Ok(snapshot)
    }
}

fn prepare_replacement(
    current: &TaskSnapshot,
    expected_revision: RevisionNumber,
    inputs: Vec<ArtifactRef>,
    now: DateTime<Utc>,
) -> Result<TaskSnapshot> {
    if current.revision != expected_revision {
        return Err(TaskStoreError::StaleRevision);
    }
    if current.status != TaskStatus::Draft {
        return Err(TaskStoreError::InvalidTransition);
    }
    if current.input_refs == inputs {
        return Err(TaskStoreError::InvalidInput);
    }
    draft::validate_content(&current.title, &current.definition, &inputs)?;
    if inputs
        .iter()
        .any(|input| input.workspace != current.reference.workspace)
    {
        return Err(TaskStoreError::ScopeMismatch);
    }
    let revision = (u64::from(current.revision) + 1)
        .try_into()
        .map_err(|_| TaskStoreError::InvalidRecord)?;
    Ok(TaskSnapshot {
        input_refs: inputs,
        revision,
        updated_at: now,
        ..current.clone()
    })
}

#[cfg(test)]
#[path = "replace_tests.rs"]
mod tests;
