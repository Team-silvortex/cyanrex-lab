//! Owner-private metadata index, separate from Artifact reads and writable content handles.
use super::*;
use crate::services::{
    task_catalog::TaskDefinitionCatalog,
    task_store::{validate_index_page_request, TaskContentIndexPage, TaskContentIndexStore},
};

/// Trusted namespace and optional frozen definitions, not a client-selected owner or resource path.
/// No Artifact namespace is supplied: listing does not prove that task content is readable.
#[derive(Clone)]
pub struct SessionTaskContentIndexWorkspace {
    namespace: String,
    scope: WorkspaceRef,
    definitions: Option<TaskDefinitionCatalog>,
}

impl SessionTaskContentIndexWorkspace {
    /// Trusted HTTP composition scopes a position-only cursor; requests cannot select this scope.
    #[cfg(unix)]
    pub(crate) fn index_scope(&self) -> WorkspaceRef {
        self.scope
    }

    pub fn new(namespace: &str, scope: WorkspaceRef) -> TaskResult<Self> {
        if !valid_namespace(namespace) {
            return Err(SessionTaskError::InvalidNamespace);
        }
        Ok(Self {
            namespace: namespace.into(),
            scope,
            definitions: None,
        })
    }

    /// Explicit neutral index of manual records and complete, configured catalogue definitions.
    pub fn from_definitions(
        namespace: &str,
        scope: WorkspaceRef,
        catalog: &TaskDefinitionCatalog,
    ) -> TaskResult<Self> {
        let mut workspace = Self::new(namespace, scope)?;
        workspace.definitions = Some(catalog.clone());
        Ok(workspace)
    }
}

impl DurableAuthSource {
    /// A committed page of metadata only, never a content read, saved-write receipt or access grant.
    pub async fn list_session_content_tasks(
        &self,
        token: &str,
        workspace: &SessionTaskContentIndexWorkspace,
        after: Option<TaskRef>,
        limit: Option<usize>,
    ) -> TaskResult<TaskContentIndexPage> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        validate_index_page_request(workspace.scope, after, limit)?;
        if workspace.scope.authority_id != self.authority_id {
            return Err(DurableAuthError::SourceMismatch.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.transaction().await?;
            let context = self
                .begin_private_work(&mut tx, token, &workspace.namespace, workspace.scope)
                .await?;
            let store = match &workspace.definitions {
                Some(catalog) => TaskContentIndexStore::from_definitions(
                    self.pool.clone(),
                    workspace.scope,
                    catalog,
                ),
                None => TaskContentIndexStore::new(self.pool.clone(), workspace.scope),
            };
            // This borrowed read shares authorization, namespace pins and commit with the source.
            // Owner identity comes only from the locked current Session, never from the cursor.
            let pending = store
                .list_in_transaction(&mut tx, context.owner, after, limit)
                .await?;
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(pending)
        })
        .await
        .map_err(|_| SessionTaskError::from(DurableAuthError::StorageUnavailable))?
    }
}
