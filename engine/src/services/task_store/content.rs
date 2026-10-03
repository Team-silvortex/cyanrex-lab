//! Trusted metadata storage only, not Artifact verification, authorization or a public save API.
use super::{records as task_records, *};
use chrono::{DateTime, Utc};

#[path = "content_commands.rs"]
mod commands;
#[path = "content_records.rs"]
mod records;

/// A stored metadata pair. This value is not a credential or evidence that referenced bytes exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskContentSnapshot {
    pub task: TaskSnapshot,
    pub manifest: TaskContentManifest,
}

/// Explicit schema-3 namespace. The inner handle cannot escape to legacy Task commands.
/// Callers must already authenticate and authorize the supplied owner and workspace.
#[derive(Clone)]
pub struct TaskContentStore {
    store: TaskStore,
}

impl TaskContentStore {
    pub fn new(pool: PgPool, scope: WorkspaceRef) -> Self {
        Self {
            store: TaskStore {
                pool,
                scope,
                storage_version: TASK_CONTENT_STORAGE_VERSION,
            },
        }
    }

    /// Installs a fresh dedicated namespace only; never adopts, migrates or repairs schema 2.
    pub async fn install_empty_namespace(&self) -> Result<()> {
        self.store.install_empty_namespace().await
    }

    pub async fn get(
        &self,
        reference: TaskRef,
        owner: PrincipalRef,
    ) -> Result<Option<TaskContentSnapshot>> {
        self.store.check_scope(reference, owner)?;
        bounded(async {
            let mut tx = self.store.transaction(true).await?;
            self.store.check_schema(&mut tx, false).await?;
            let result = self.read(&mut tx, reference, owner, false).await?;
            tx.commit().await?;
            Ok(result)
        })
        .await
    }

    /// Pending locked read on the source's transaction. No connection acquisition or commit.
    #[cfg(unix)]
    pub(crate) async fn get_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        reference: TaskRef,
        owner: PrincipalRef,
    ) -> Result<Option<TaskContentSnapshot>> {
        self.store.check_scope(reference, owner)?;
        self.store.check_schema(tx, true).await?;
        self.read(tx, reference, owner, true).await
    }

    fn check_manifest_scope(&self, manifest: &TaskContentManifest) -> Result<()> {
        if manifest
            .payload()
            .iter()
            .any(|item| item.artifact().workspace != self.store.scope)
        {
            return Err(TaskStoreError::ScopeMismatch);
        }
        Ok(())
    }

    fn validate_record(&self, snapshot: &TaskContentSnapshot) -> Result<()> {
        self.store.validate_snapshot(&snapshot.task)?;
        if snapshot.task.definition.is_some() {
            return Err(TaskStoreError::InvalidRecord);
        }
        snapshot
            .manifest
            .validate_task_snapshot(&snapshot.task)
            .map_err(|_| TaskStoreError::InvalidRecord)
    }
}

fn next_revision(revision: RevisionNumber) -> Result<RevisionNumber> {
    (u64::from(revision) + 1)
        .try_into()
        .map_err(|_| TaskStoreError::InvalidRecord)
}

fn prepare_replacement(
    current: &TaskContentSnapshot,
    expected: RevisionNumber,
    manifest: TaskContentManifest,
    now: DateTime<Utc>,
) -> Result<TaskContentSnapshot> {
    if current.task.revision != expected {
        return Err(TaskStoreError::StaleRevision);
    }
    if current.task.status != TaskStatus::Draft {
        return Err(TaskStoreError::InvalidTransition);
    }
    if current.manifest == manifest {
        return Err(TaskStoreError::InvalidInput);
    }
    if manifest
        .payload()
        .iter()
        .any(|item| item.artifact().workspace != current.task.reference.workspace)
    {
        return Err(TaskStoreError::ScopeMismatch);
    }
    Ok(TaskContentSnapshot {
        task: TaskSnapshot {
            title: manifest.title().into(),
            input_refs: manifest.input_refs(),
            revision: next_revision(current.task.revision)?,
            updated_at: now,
            ..current.task.clone()
        },
        manifest,
    })
}

fn prepare_transition(
    current: &TaskContentSnapshot,
    expected: RevisionNumber,
    status: TaskStatus,
    now: DateTime<Utc>,
) -> Result<TaskContentSnapshot> {
    if current.task.revision != expected {
        return Err(TaskStoreError::StaleRevision);
    }
    if !current.task.status.can_transition_to(status) {
        return Err(TaskStoreError::InvalidTransition);
    }
    Ok(TaskContentSnapshot {
        task: TaskSnapshot {
            revision: next_revision(current.task.revision)?,
            status,
            updated_at: now,
            ..current.task.clone()
        },
        manifest: current.manifest.clone(),
    })
}

#[cfg(test)]
#[path = "content_tests.rs"]
mod tests;
