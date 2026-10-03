//! Current-Session schema-3 content commands, not a public route, upload or sharing authority.
use super::*;
use crate::{
    models::collaboration::{
        ArtifactContent, PrincipalRef, TaskContentManifest, TextPayloadBinding,
    },
    services::{
        artifact_store::ArtifactStoreError,
        task_content::validate_text_payload_content,
        task_store::{TaskContentSnapshot, TaskContentStore},
    },
};

/// Trusted routing for separate initialized Task/Artifact namespaces with the same workspace.
/// No request deserializer, caller-selected owner or second transaction owner.
#[derive(Clone)]
pub struct SessionTaskContentWorkspace {
    namespace: String,
    scope: WorkspaceRef,
    artifacts: SessionArtifactWorkspace,
}

impl SessionTaskContentWorkspace {
    pub fn new(
        namespace: &str,
        scope: WorkspaceRef,
        artifacts: SessionArtifactWorkspace,
    ) -> TaskResult<Self> {
        if !valid_namespace(namespace) || namespace == artifacts.namespace {
            return Err(SessionTaskError::InvalidNamespace);
        }
        if scope != artifacts.scope {
            return Err(TaskStoreError::ScopeMismatch.into());
        }
        Ok(Self {
            namespace: namespace.into(),
            scope,
            artifacts,
        })
    }

    fn check_manifest_scope(&self, manifest: &TaskContentManifest) -> TaskResult<()> {
        if manifest
            .payload()
            .iter()
            .any(|item| item.artifact().workspace != self.scope)
        {
            return Err(TaskStoreError::ScopeMismatch.into());
        }
        Ok(())
    }

    /// One global lock order for the entire old/new union. Duplicate coordinates are retained:
    /// a valid old digest must never hide a conflicting new pin. Only reads keep body buffers.
    async fn verify_contents(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        owner: PrincipalRef,
        checks: &[(TextPayloadBinding, TaskStoreError)],
        retain: bool,
    ) -> TaskResult<Vec<ArtifactContent>> {
        self.artifacts
            .store
            .validate_namespace_in_transaction(tx)
            .await?;
        let mut ordered: Vec<_> = checks.iter().enumerate().collect();
        ordered.sort_by_key(|(_, (item, _))| {
            (
                item.artifact().artifact_id.as_uuid(),
                item.artifact().revision_id.as_uuid(),
            )
        });
        let mut contents = Vec::new();
        for (index, (item, invalid)) in ordered {
            let content = self
                .artifacts
                .store
                .read_in_transaction(tx, item.artifact(), owner)
                .await?
                .ok_or(ArtifactStoreError::NotFound)?;
            validate_text_payload_content(item, owner, &content).map_err(|_| *invalid)?;
            if retain {
                contents.push((index, content));
            }
        }
        contents.sort_by_key(|(index, _)| *index);
        Ok(contents.into_iter().map(|(_, content)| content).collect())
    }
}

/// Committed metadata and owned bytes, not a reusable grant for later writes or Review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTaskContent {
    pub snapshot: TaskContentSnapshot,
    pub contents: Vec<ArtifactContent>,
}

enum ContentOperation {
    Create(TaskContentManifest),
    Read,
    Replace(RevisionNumber, TaskContentManifest),
    Transition(RevisionNumber, TaskStatus),
}
enum ContentPending {
    Snapshot(TaskContentSnapshot),
    Read(Option<SessionTaskContent>),
}

fn content_checks(
    old: Option<&TaskContentSnapshot>,
    operation: &ContentOperation,
) -> TaskResult<Vec<(TextPayloadBinding, TaskStoreError)>> {
    let mut checks = Vec::new();
    match operation {
        ContentOperation::Create(_) if old.is_some() => return Err(TaskStoreError::Conflict.into()),
        ContentOperation::Create(_) => {}
        _ => {
            let old = old.ok_or(TaskStoreError::NotFound)?;
            match operation {
                ContentOperation::Replace(revision, manifest) => {
                    if old.task.revision != *revision {
                        return Err(TaskStoreError::StaleRevision.into());
                    }
                    if old.task.status != TaskStatus::Draft {
                        return Err(TaskStoreError::InvalidTransition.into());
                    }
                    if old.manifest == *manifest {
                        return Err(TaskStoreError::InvalidInput.into());
                    }
                }
                ContentOperation::Transition(revision, status) => {
                    if old.task.revision != *revision {
                        return Err(TaskStoreError::StaleRevision.into());
                    }
                    if !old.task.status.can_transition_to(*status) {
                        return Err(TaskStoreError::InvalidTransition.into());
                    }
                }
                _ => {}
            }
            checks.extend(
                old.manifest
                    .payload()
                    .iter()
                    .cloned()
                    .map(|item| (item, TaskStoreError::InvalidRecord)),
            );
        }
    }
    if let ContentOperation::Create(manifest) | ContentOperation::Replace(_, manifest) = operation {
        checks.extend(
            manifest
                .payload()
                .iter()
                .cloned()
                .map(|item| (item, TaskStoreError::InvalidInput)),
        );
    }
    Ok(checks)
}

impl DurableAuthSource {
    pub async fn create_session_content_task(
        &self,
        token: &str,
        workspace: &SessionTaskContentWorkspace,
        id: TaskId,
        manifest: TaskContentManifest,
    ) -> TaskResult<TaskContentSnapshot> {
        workspace.check_manifest_scope(&manifest)?;
        match self
            .session_content_task(token, workspace, id, ContentOperation::Create(manifest))
            .await?
        {
            ContentPending::Snapshot(snapshot) => Ok(snapshot),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    pub async fn get_session_content_task(
        &self,
        token: &str,
        workspace: &SessionTaskContentWorkspace,
        id: TaskId,
    ) -> TaskResult<Option<SessionTaskContent>> {
        match self
            .session_content_task(token, workspace, id, ContentOperation::Read)
            .await?
        {
            ContentPending::Read(result) => Ok(result),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    pub async fn replace_session_content_task(
        &self,
        token: &str,
        workspace: &SessionTaskContentWorkspace,
        id: TaskId,
        revision: RevisionNumber,
        manifest: TaskContentManifest,
    ) -> TaskResult<TaskContentSnapshot> {
        workspace.check_manifest_scope(&manifest)?;
        match self
            .session_content_task(
                token,
                workspace,
                id,
                ContentOperation::Replace(revision, manifest),
            )
            .await?
        {
            ContentPending::Snapshot(snapshot) => Ok(snapshot),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    /// Even cancellation checks the saved exact content and current Session; it is not cleanup.
    pub async fn transition_session_content_task(
        &self,
        token: &str,
        workspace: &SessionTaskContentWorkspace,
        id: TaskId,
        revision: RevisionNumber,
        status: TaskStatus,
    ) -> TaskResult<TaskContentSnapshot> {
        match self
            .session_content_task(
                token,
                workspace,
                id,
                ContentOperation::Transition(revision, status),
            )
            .await?
        {
            ContentPending::Snapshot(snapshot) => Ok(snapshot),
            _ => Err(TaskStoreError::InvalidRecord.into()),
        }
    }

    async fn session_content_task(
        &self,
        token: &str,
        workspace: &SessionTaskContentWorkspace,
        id: TaskId,
        operation: ContentOperation,
    ) -> TaskResult<ContentPending> {
        if !valid_token(token) {
            return Err(SessionCommandError::InvalidSession.into());
        }
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut tx = self.transaction().await?;
            let mut context = self
                .begin_private_work(&mut tx, token, &workspace.namespace, workspace.scope)
                .await?;
            let owner = context.owner;
            let reference = TaskRef {
                workspace: workspace.scope,
                task_id: id,
            };
            let store = TaskContentStore::new(self.pool.clone(), workspace.scope);
            // Lock schema-3 metadata and the owned Task BEFORE inspecting any Artifact. This
            // handle's helpers borrow the source transaction, never its own pool or a v2 handle.
            let old = store.get_in_transaction(&mut tx, reference, owner).await?;
            if matches!(&operation, ContentOperation::Read) && old.is_none() {
                context.finish(self, &mut tx).await?;
                tx.commit().await?;
                return Ok(ContentPending::Read(None));
            }
            let checks = content_checks(old.as_ref(), &operation)?;
            // Pin the Artifact namespace even for a named empty task or deleting the last item.
            context
                .select_target(&mut tx, &workspace.artifacts.namespace)
                .await?;
            let read = matches!(&operation, ContentOperation::Read);
            let contents = workspace
                .verify_contents(&mut tx, owner, &checks, read)
                .await?;
            context.select_target(&mut tx, &workspace.namespace).await?;
            let snapshot = match operation {
                ContentOperation::Create(manifest) => {
                    store
                        .create_in_transaction(&mut tx, id, owner, manifest)
                        .await?
                }
                ContentOperation::Read => old.ok_or(TaskStoreError::NotFound)?,
                ContentOperation::Replace(revision, manifest) => {
                    store
                        .replace_in_transaction(&mut tx, reference, owner, revision, manifest)
                        .await?
                }
                ContentOperation::Transition(revision, status) => {
                    store
                        .transition_in_transaction(&mut tx, reference, owner, revision, status)
                        .await?
                }
            };
            if !read {
                // Recheck ALL old/new bytes after Task/outbox triggers, not just the new pins.
                context
                    .select_target(&mut tx, &workspace.artifacts.namespace)
                    .await?;
                workspace
                    .verify_contents(&mut tx, owner, &checks, false)
                    .await?;
                context.select_target(&mut tx, &workspace.namespace).await?;
            }
            if store
                .get_in_transaction(&mut tx, reference, owner)
                .await?
                .as_ref()
                != Some(&snapshot)
            {
                return Err(TaskStoreError::InvalidRecord.into());
            }
            // No retained bytes or metadata leave before fresh Session/membership and a confirmed
            // source-owned commit. Timeout/cancellation does not prove rollback or allow cleanup.
            context.finish(self, &mut tx).await?;
            tx.commit().await?;
            Ok(if read {
                ContentPending::Read(Some(SessionTaskContent { snapshot, contents }))
            } else {
                ContentPending::Snapshot(snapshot)
            })
        })
        .await
        .map_err(|_| SessionTaskError::from(DurableAuthError::StorageUnavailable))?
    }
}
