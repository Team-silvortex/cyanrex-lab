//! Explicit catalogue-content composition; not the manual publication or HTTP workspace.
use super::*;
use crate::models::collaboration::TaskDefinition;

/// Trusted frozen metadata and content routing; no provider or client deserialization.
/// This distinct handle cannot enter the manual HTTP or draft-publication composition.
#[derive(Clone)]
pub struct SessionCatalogTaskContentWorkspace {
    content: SessionTaskContentWorkspace,
    definitions: TaskDefinitionCatalog,
}

impl SessionCatalogTaskContentWorkspace {
    pub(in crate::services::auth_service::durable_source) fn publication_scope(
        &self,
    ) -> WorkspaceRef {
        self.content.publication_scope()
    }

    pub(in crate::services::auth_service::durable_source) fn publication_artifacts(
        &self,
    ) -> &SessionArtifactWorkspace {
        self.content.publication_artifacts()
    }

    pub(in crate::services::auth_service::durable_source) fn publication_definition(
        &self,
        reference: &TaskDefinitionRef,
    ) -> Option<&TaskDefinition> {
        self.definitions.get(reference)
    }

    pub fn from_definitions(
        namespace: &str,
        scope: WorkspaceRef,
        artifacts: SessionArtifactWorkspace,
        catalog: &TaskDefinitionCatalog,
    ) -> TaskResult<Self> {
        Ok(Self {
            content: SessionTaskContentWorkspace::new(namespace, scope, artifacts)?,
            definitions: catalog.clone(),
        })
    }
}

impl DurableAuthSource {
    pub async fn create_session_catalog_content_task(
        &self,
        token: &str,
        workspace: &SessionCatalogTaskContentWorkspace,
        id: TaskId,
        reference: &TaskDefinitionRef,
        manifest: TaskContentManifest,
    ) -> TaskResult<TaskContentSnapshot> {
        workspace.content.check_manifest_scope(&manifest)?;
        workspace
            .definitions
            .get(reference)
            .ok_or(TaskStoreError::UnknownDefinition)?;
        match self
            .session_content_task(
                token,
                &workspace.content,
                id,
                ContentOperation::CatalogCreate(reference.clone(), manifest),
                Some(&workspace.definitions),
            )
            .await?
        {
            ContentPending::Snapshot(snapshot) => Ok(snapshot),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    pub async fn get_session_catalog_content_task(
        &self,
        token: &str,
        workspace: &SessionCatalogTaskContentWorkspace,
        id: TaskId,
    ) -> TaskResult<Option<SessionTaskContent>> {
        match self
            .session_content_task(
                token,
                &workspace.content,
                id,
                ContentOperation::Read,
                Some(&workspace.definitions),
            )
            .await?
        {
            ContentPending::Read(content) => Ok(content),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    pub async fn replace_session_catalog_content_task(
        &self,
        token: &str,
        workspace: &SessionCatalogTaskContentWorkspace,
        id: TaskId,
        revision: RevisionNumber,
        manifest: TaskContentManifest,
    ) -> TaskResult<TaskContentSnapshot> {
        workspace.content.check_manifest_scope(&manifest)?;
        match self
            .session_content_task(
                token,
                &workspace.content,
                id,
                ContentOperation::Replace(revision, manifest),
                Some(&workspace.definitions),
            )
            .await?
        {
            ContentPending::Snapshot(snapshot) => Ok(snapshot),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    pub async fn transition_session_catalog_content_task(
        &self,
        token: &str,
        workspace: &SessionCatalogTaskContentWorkspace,
        id: TaskId,
        revision: RevisionNumber,
        status: TaskStatus,
    ) -> TaskResult<TaskContentSnapshot> {
        match self
            .session_content_task(
                token,
                &workspace.content,
                id,
                ContentOperation::Transition(revision, status),
                Some(&workspace.definitions),
            )
            .await?
        {
            ContentPending::Snapshot(snapshot) => Ok(snapshot),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }
}
