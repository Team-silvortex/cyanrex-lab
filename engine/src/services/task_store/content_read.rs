//! Strict locked content metadata on a caller-owned transaction; no pool or commit.
use super::*;
use crate::services::task_catalog::TaskDefinitionCatalog;

pub(crate) async fn get_content_in_transaction(
    tx: &mut Tx<'_>,
    scope: WorkspaceRef,
    definitions: Option<&TaskDefinitionCatalog>,
    reference: TaskRef,
    owner: PrincipalRef,
) -> Result<Option<TaskContentSnapshot>> {
    check_scope(scope, reference, owner)?;
    schema::check_schema_in_transaction(tx, scope, TASK_CONTENT_STORAGE_VERSION, true).await?;
    content::read_content_record(tx, scope, definitions, reference, owner, true).await
}
