//! Pool-free pending replacement. Current authority, byte checks and COMMIT belong to the caller.
use super::*;

fn check_replace_input(
    scope: WorkspaceRef,
    reference: TaskRef,
    owner: PrincipalRef,
    manifest: &TaskContentManifest,
) -> Result<()> {
    check_scope(scope, reference, owner)?;
    check_manifest_scope(scope, manifest)
}

/// Manual schema-3 metadata replacement only; no Artifact publication or confirmed save result.
pub(crate) async fn replace_manual_content_in_transaction(
    tx: &mut Tx<'_>,
    scope: WorkspaceRef,
    reference: TaskRef,
    owner: PrincipalRef,
    expected_revision: RevisionNumber,
    manifest: TaskContentManifest,
) -> Result<TaskContentSnapshot> {
    replace_content_in_transaction(
        tx,
        scope,
        None,
        reference,
        owner,
        expected_revision,
        manifest,
    )
    .await
}

/// Existing manual and catalogue handles retain the same validation order and SQL policy.
pub(super) async fn replace_content_in_transaction(
    tx: &mut Tx<'_>,
    scope: WorkspaceRef,
    definitions: Option<&TaskDefinitionCatalog>,
    reference: TaskRef,
    owner: PrincipalRef,
    expected_revision: RevisionNumber,
    manifest: TaskContentManifest,
) -> Result<TaskContentSnapshot> {
    check_replace_input(scope, reference, owner, &manifest)?;
    schema::check_schema_in_transaction(tx, scope, TASK_CONTENT_STORAGE_VERSION, true).await?;
    let current = records::read_content_record(tx, scope, definitions, reference, owner, true)
        .await?
        .ok_or(TaskStoreError::NotFound)?;
    let snapshot = prepare_replacement(&current, expected_revision, manifest, Utc::now())?;
    records::update_and_confirm(tx, scope, definitions, &snapshot, expected_revision).await?;
    Ok(snapshot)
}

#[cfg(test)]
#[path = "content_replace_tests.rs"]
mod tests;
