//! Explicit definition/content composition, never implicit admission through the manual handle.
use super::*;

impl TaskContentStore {
    /// Frozen metadata only; no evaluator, discovery or lookup from caller-supplied definitions.
    pub fn from_definitions(
        pool: PgPool,
        scope: WorkspaceRef,
        catalog: &TaskDefinitionCatalog,
    ) -> Self {
        let mut store = Self::new(pool, scope);
        store.definitions = Some(catalog.clone());
        store
    }

    fn catalog_definition(&self, reference: &TaskDefinitionRef) -> Result<TaskDefinition> {
        self.definitions
            .as_ref()
            .ok_or(TaskStoreError::InvalidInput)?
            .get(reference)
            .cloned()
            .ok_or(TaskStoreError::UnknownDefinition)
    }

    /// Creates content with an exact definition selected from the trusted frozen catalogue.
    pub async fn create_catalog(
        &self,
        task_id: TaskId,
        owner: PrincipalRef,
        reference: &TaskDefinitionRef,
        manifest: TaskContentManifest,
    ) -> Result<TaskContentSnapshot> {
        self.store.check_scope(
            TaskRef {
                workspace: self.store.scope,
                task_id,
            },
            owner,
        )?;
        self.check_manifest_scope(&manifest)?;
        // Reject unknown coordinates before opening a transaction. Exact metadata is selected
        // again in the borrowed helper; immutable clones retain no executable provider.
        self.catalog_definition(reference)?;
        bounded(async {
            let mut tx = self.store.transaction(false).await?;
            let snapshot = self
                .create_catalog_in_transaction(&mut tx, task_id, owner, reference, manifest)
                .await?;
            tx.commit().await?;
            Ok(snapshot)
        })
        .await
    }

    /// Pending result only; the source retains its original Session guard and final commit.
    pub(crate) async fn create_catalog_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        task_id: TaskId,
        owner: PrincipalRef,
        reference: &TaskDefinitionRef,
        manifest: TaskContentManifest,
    ) -> Result<TaskContentSnapshot> {
        self.store.check_scope(
            TaskRef {
                workspace: self.store.scope,
                task_id,
            },
            owner,
        )?;
        self.check_manifest_scope(&manifest)?;
        let definition = self.catalog_definition(reference)?;
        self.create_selected_in_transaction(tx, task_id, owner, manifest, Some(definition))
            .await
    }
}
