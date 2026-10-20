//! Pool-free pending creation. The caller owns current authority, deadlines and final COMMIT.
use super::*;

fn check_create_input(
    scope: WorkspaceRef,
    task_id: TaskId,
    owner: PrincipalRef,
    manifest: &TaskContentManifest,
) -> Result<()> {
    check_scope(
        scope,
        TaskRef {
            workspace: scope,
            task_id,
        },
        owner,
    )?;
    check_manifest_scope(scope, manifest)
}

/// Manual schema-3 content only, not Artifact publication or a confirmed save result.
pub(crate) async fn create_manual_content_in_transaction(
    tx: &mut Tx<'_>,
    scope: WorkspaceRef,
    task_id: TaskId,
    owner: PrincipalRef,
    manifest: TaskContentManifest,
) -> Result<TaskContentSnapshot> {
    create_selected_in_transaction(tx, scope, None, task_id, owner, manifest, None).await
}

/// Existing manual and catalogue stores share their original write and validation policy.
pub(super) async fn create_selected_in_transaction(
    tx: &mut Tx<'_>,
    scope: WorkspaceRef,
    definitions: Option<&TaskDefinitionCatalog>,
    task_id: TaskId,
    owner: PrincipalRef,
    manifest: TaskContentManifest,
    definition: Option<TaskDefinition>,
) -> Result<TaskContentSnapshot> {
    check_create_input(scope, task_id, owner, &manifest)?;
    schema::check_schema_in_transaction(tx, scope, TASK_CONTENT_STORAGE_VERSION, true).await?;
    let now = Utc::now();
    let snapshot = TaskContentSnapshot {
        task: TaskSnapshot {
            schema_version: CoreSchemaVersion,
            reference: TaskRef {
                workspace: scope,
                task_id,
            },
            owner,
            title: manifest.title().into(),
            definition,
            input_refs: manifest.input_refs(),
            status: TaskStatus::Draft,
            revision: 1.try_into().map_err(|_| TaskStoreError::InvalidInput)?,
            created_at: now,
            updated_at: now,
        },
        manifest,
    };
    validate_content_record(scope, definitions, &snapshot)?;
    let (raw, metadata) = records::encode(&snapshot)?;
    confirmed_one(
        sqlx::query(
            "INSERT INTO collaboration_tasks
            (task_id, owner_id, revision, snapshot, manifest) VALUES ($1, $2, 1, $3, $4)",
        )
        .bind(task_id.as_uuid())
        .bind(owner.principal_id.as_uuid())
        .bind(&raw)
        .bind(&metadata)
        .execute(&mut **tx)
        .await?
        .rows_affected(),
    )?;
    records::append_and_confirm(tx, scope, definitions, &snapshot, &raw, &metadata).await?;
    Ok(snapshot)
}

#[cfg(test)]
#[path = "content_create_tests.rs"]
mod tests;
