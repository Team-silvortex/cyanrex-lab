//! Owner-private metadata index, separate from Artifact reads and writable content handles.
use super::super::task_index_transaction::PreparedTaskIndexRead;
use super::*;
use crate::services::{task_catalog::TaskDefinitionCatalog, task_store::TaskContentIndexPage};

/// Trusted namespace and optional frozen definitions, not a client-selected owner or resource path.
/// No Artifact namespace is supplied: listing does not prove that task content is readable.
#[derive(Clone)]
pub struct SessionTaskContentIndexWorkspace {
    namespace: String,
    scope: WorkspaceRef,
    definitions: Option<TaskDefinitionCatalog>,
}

impl SessionTaskContentIndexWorkspace {
    pub(in super::super) fn transaction_config(
        &self,
    ) -> (&str, WorkspaceRef, Option<&TaskDefinitionCatalog>) {
        (&self.namespace, self.scope, self.definitions.as_ref())
    }

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
        PreparedTaskIndexRead::validate(self.authority_id, token, workspace, after, limit)?;
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.transaction().await?;
            let prepared = PreparedTaskIndexRead::prepare(
                self.authority_id,
                &mut tx,
                token,
                workspace,
                after,
                limit,
            )
            .await?;
            let pending = prepared.finish(&mut tx).await?;
            tx.commit().await?;
            Ok(pending)
        })
        .await
        .map_err(|_| SessionTaskError::from(DurableAuthError::StorageUnavailable))?
    }
}
