//! Explicit owner-private metadata indexing, never content verification or writable authority.
use super::*;
use crate::services::task_catalog::TaskDefinitionCatalog;
use chrono::{DateTime, Utc};

/// Metadata only: no owner, manifest, Artifact pins or bytes. An entry is not a read/write grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskContentIndexEntry {
    pub reference: TaskRef,
    pub revision: RevisionNumber,
    pub title: String,
    pub status: TaskStatus,
    pub definition_ref: Option<TaskDefinitionRef>,
    pub updated_at: DateTime<Utc>,
    pub payload_items: usize,
}

/// A bounded current metadata page, not a stable cross-request snapshot or complete inventory.
/// The cursor is a position only; it confers no ownership, existence or authorization proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskContentIndexPage {
    pub entries: Vec<TaskContentIndexEntry>,
    pub next_after: Option<TaskRef>,
}

/// Separate read-only schema-3 consumer. No installer, writable handle or Artifact access.
/// Trusted callers must already authorize the supplied owner. Session adapters own their guard.
#[derive(Clone)]
pub struct TaskContentIndexStore {
    pool: PgPool,
    scope: WorkspaceRef,
    definitions: Option<TaskDefinitionCatalog>,
}

/// Shape admission only; a cursor is not a claim that its target exists or belongs to the owner.
pub(crate) fn validate_index_page_request(
    scope: WorkspaceRef,
    after: Option<TaskRef>,
    limit: Option<usize>,
) -> Result<usize> {
    if after.is_some_and(|reference| reference.workspace != scope) {
        return Err(TaskStoreError::ScopeMismatch);
    }
    let limit = limit.unwrap_or(20);
    if !(1..=32).contains(&limit) {
        return Err(TaskStoreError::InvalidInput);
    }
    Ok(limit)
}

impl TaskContentIndexStore {
    /// Manual-only metadata admission; catalogue records are not silently skipped.
    pub fn new(pool: PgPool, scope: WorkspaceRef) -> Self {
        Self {
            pool,
            scope,
            definitions: None,
        }
    }

    /// Explicitly admits manual work and complete definitions from this frozen metadata set.
    pub fn from_definitions(
        pool: PgPool,
        scope: WorkspaceRef,
        catalog: &TaskDefinitionCatalog,
    ) -> Self {
        Self {
            pool,
            scope,
            definitions: Some(catalog.clone()),
        }
    }

    /// Defaults to 20 entries, bounded to 1–32. Every selected row, including lookahead, is checked.
    pub async fn list(
        &self,
        owner: PrincipalRef,
        after: Option<TaskRef>,
        limit: Option<usize>,
    ) -> Result<TaskContentIndexPage> {
        let limit = check_request(self.scope, owner, after, limit)?;
        bounded(async {
            let store = self.schema_store();
            let mut tx = store.transaction(true).await?;
            let page = read_page(
                &mut tx,
                self.scope,
                self.definitions.as_ref(),
                owner,
                after,
                limit,
                false,
            )
            .await?;
            tx.commit().await?;
            Ok(page)
        })
        .await
    }

    fn schema_store(&self) -> TaskStore {
        TaskStore {
            pool: self.pool.clone(),
            scope: self.scope,
            storage_version: TASK_CONTENT_STORAGE_VERSION,
        }
    }
}

/// Pending locked metadata only. The caller owns current authorization, guards and COMMIT.
pub(crate) async fn read_content_index_in_transaction(
    tx: &mut Tx<'_>,
    scope: WorkspaceRef,
    definitions: Option<&TaskDefinitionCatalog>,
    owner: PrincipalRef,
    after: Option<TaskRef>,
    limit: Option<usize>,
) -> Result<TaskContentIndexPage> {
    let limit = check_request(scope, owner, after, limit)?;
    read_page(tx, scope, definitions, owner, after, limit, true).await
}

fn check_request(
    scope: WorkspaceRef,
    owner: PrincipalRef,
    after: Option<TaskRef>,
    limit: Option<usize>,
) -> Result<usize> {
    if owner.authority_id != scope.authority_id {
        return Err(TaskStoreError::ScopeMismatch);
    }
    validate_index_page_request(scope, after, limit)
}

async fn read_page(
    tx: &mut Tx<'_>,
    scope: WorkspaceRef,
    definitions: Option<&TaskDefinitionCatalog>,
    owner: PrincipalRef,
    after: Option<TaskRef>,
    limit: usize,
    lock: bool,
) -> Result<TaskContentIndexPage> {
    schema::check_schema_in_transaction(tx, scope, TASK_CONTENT_STORAGE_VERSION, lock).await?;
    // Refer to every required reader column even when this owner has no selected rows.
    // These fixed false predicates never fetch another owner's metadata or install/repair.
    sqlx::query(
        "SELECT task_id, owner_id, revision, snapshot, manifest
            FROM collaboration_tasks WHERE FALSE",
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "SELECT event_id, task_id, revision, actor_id, event_type, snapshot, manifest
            FROM collaboration_task_outbox WHERE FALSE",
    )
    .execute(&mut **tx)
    .await?;
    // Scope comes from the pinned namespace/schema; each full record checks it again.
    // Never filter JSON fields: malformed owned records must not disappear from the page.
    let statement = if lock {
        "SELECT task_id, CASE WHEN octet_length(snapshot) <= 32768 THEN snapshot END AS snapshot
            FROM collaboration_tasks WHERE owner_id = $1 AND ($2::uuid IS NULL OR task_id > $2)
            ORDER BY task_id LIMIT $3 FOR UPDATE"
    } else {
        "SELECT task_id, CASE WHEN octet_length(snapshot) <= 32768 THEN snapshot END AS snapshot
            FROM collaboration_tasks WHERE owner_id = $1 AND ($2::uuid IS NULL OR task_id > $2)
            ORDER BY task_id LIMIT $3"
    };
    let rows = sqlx::query(statement)
        .bind(owner.principal_id.as_uuid())
        .bind(after.map(|reference| reference.task_id.as_uuid()))
        .bind((limit + 1) as i64)
        .fetch_all(&mut **tx)
        .await?;
    let has_more = rows.len() > limit;
    let mut previous = after.map(|reference| reference.task_id);
    let mut entries = Vec::with_capacity(limit);
    for row in rows {
        let id = TaskId::try_from(row.try_get::<uuid::Uuid, _>("task_id")?)
            .map_err(|_| TaskStoreError::InvalidRecord)?;
        if previous.is_some_and(|previous| id <= previous) {
            return Err(TaskStoreError::InvalidRecord);
        }
        previous = Some(id);
        let raw: Option<String> = row.try_get("snapshot")?;
        let selected: TaskSnapshot =
            serde_json::from_str(&raw.ok_or(TaskStoreError::InvalidRecord)?)
                .map_err(|_| TaskStoreError::InvalidRecord)?;
        // Inspect only bounded metadata to select the unchanged strict reader. An unbound
        // catalogue definition is still Some and never becomes a manual fallback.
        let selected_definitions = match &selected.definition {
            None => None,
            Some(_) => Some(definitions.ok_or(TaskStoreError::InvalidRecord)?),
        };
        let reference = TaskRef {
            workspace: scope,
            task_id: id,
        };
        let snapshot =
            content::read_content_record(tx, scope, selected_definitions, reference, owner, lock)
                .await?
                .ok_or(TaskStoreError::InvalidRecord)?;
        if snapshot.task != selected {
            return Err(TaskStoreError::InvalidRecord);
        }
        // The lookahead is fully validated too, but never included or used as the cursor.
        if entries.len() < limit {
            entries.push(TaskContentIndexEntry {
                reference: snapshot.task.reference,
                revision: snapshot.task.revision,
                title: snapshot.task.title,
                status: snapshot.task.status,
                definition_ref: snapshot
                    .task
                    .definition
                    .map(|definition| definition.reference),
                updated_at: snapshot.task.updated_at,
                payload_items: snapshot.manifest.payload().len(),
            });
        }
    }
    let next_after = if has_more {
        entries.last().map(|entry| entry.reference)
    } else {
        None
    };
    Ok(TaskContentIndexPage {
        entries,
        next_after,
    })
}
