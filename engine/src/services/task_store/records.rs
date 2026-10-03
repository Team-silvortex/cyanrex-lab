use super::*;

pub(super) fn event_type(revision: RevisionNumber) -> &'static str {
    if u64::from(revision) == 1 {
        "cyanrex.task.created"
    } else {
        "cyanrex.task.status_changed"
    }
}

impl TaskStore {
    pub(super) fn validate_snapshot(&self, snapshot: &TaskSnapshot) -> Result<()> {
        self.check_scope(snapshot.reference, snapshot.owner)
            .map_err(|_| TaskStoreError::InvalidRecord)?;
        draft::validate_content(&snapshot.title, &snapshot.definition, &snapshot.input_refs)
            .map_err(|_| TaskStoreError::InvalidRecord)?;
        if snapshot
            .input_refs
            .iter()
            .any(|input| input.workspace != self.scope)
            || (u64::from(snapshot.revision) == 1 && snapshot.status != TaskStatus::Draft)
            || (u64::from(snapshot.revision) > 1 && snapshot.status == TaskStatus::Draft)
        {
            return Err(TaskStoreError::InvalidRecord);
        }
        Ok(())
    }

    pub(super) async fn read(
        &self,
        tx: &mut Tx<'_>,
        reference: TaskRef,
        owner: PrincipalRef,
        lock: bool,
    ) -> Result<Option<TaskSnapshot>> {
        let statement = if lock {
            "SELECT owner_id, revision, CASE WHEN octet_length(snapshot) <= 32768 THEN snapshot END AS snapshot
             FROM collaboration_tasks WHERE task_id = $1 AND owner_id = $2 FOR UPDATE"
        } else {
            "SELECT owner_id, revision, CASE WHEN octet_length(snapshot) <= 32768 THEN snapshot END AS snapshot
             FROM collaboration_tasks WHERE task_id = $1 AND owner_id = $2"
        };
        let Some(row) = sqlx::query(statement)
            .bind(reference.task_id.as_uuid())
            .bind(owner.principal_id.as_uuid())
            .fetch_optional(&mut **tx)
            .await?
        else {
            return Ok(None);
        };
        let raw: Option<String> = row.try_get("snapshot")?;
        let snapshot: TaskSnapshot =
            serde_json::from_str(&raw.ok_or(TaskStoreError::InvalidRecord)?)
                .map_err(|_| TaskStoreError::InvalidRecord)?;
        self.validate_snapshot(&snapshot)?;
        if snapshot.reference != reference
            || snapshot.owner != owner
            || row.try_get::<i64, _>("revision")? != u64::from(snapshot.revision) as i64
        {
            return Err(TaskStoreError::InvalidRecord);
        }
        // A separate statement after FOR UPDATE sees a concurrent winner's committed event.
        // Read-only callers instead use one repeatable-read snapshot for both records.
        let event = sqlx::query(
            "SELECT event_id, revision, actor_id, event_type,
            CASE WHEN octet_length(snapshot) <= 32768 THEN snapshot END AS snapshot
            FROM collaboration_task_outbox WHERE task_id = $1 ORDER BY revision DESC LIMIT 1",
        )
        .bind(reference.task_id.as_uuid())
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(TaskStoreError::InvalidRecord)?;
        let raw: Option<String> = event.try_get("snapshot")?;
        let recorded: TaskSnapshot =
            serde_json::from_str(&raw.ok_or(TaskStoreError::InvalidRecord)?)
                .map_err(|_| TaskStoreError::InvalidRecord)?;
        if recorded != snapshot
            || event.try_get::<uuid::Uuid, _>("event_id")?.is_nil()
            || event.try_get::<uuid::Uuid, _>("actor_id")? != owner.principal_id.as_uuid()
            || event.try_get::<i64, _>("revision")? != u64::from(snapshot.revision) as i64
            || event.try_get::<String, _>("event_type")? != event_type(snapshot.revision)
        {
            return Err(TaskStoreError::InvalidRecord);
        }
        Ok(Some(snapshot))
    }

    pub(super) async fn append_and_confirm(
        &self,
        tx: &mut Tx<'_>,
        snapshot: &TaskSnapshot,
        raw: &str,
    ) -> Result<()> {
        confirmed_one(sqlx::query("INSERT INTO collaboration_task_outbox
            (event_id, task_id, revision, actor_id, event_type, snapshot) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(uuid::Uuid::new_v4()).bind(snapshot.reference.task_id.as_uuid())
            .bind(u64::from(snapshot.revision) as i64).bind(snapshot.owner.principal_id.as_uuid())
            .bind(event_type(snapshot.revision)).bind(raw).execute(&mut **tx).await?.rows_affected())?;
        self.check_schema(tx, true).await?;
        if self
            .read(tx, snapshot.reference, snapshot.owner, true)
            .await?
            .as_ref()
            != Some(snapshot)
        {
            return Err(TaskStoreError::InvalidRecord);
        }
        Ok(())
    }
}

pub(super) fn encode(snapshot: &TaskSnapshot) -> Result<String> {
    let raw = serde_json::to_string(snapshot).map_err(|_| TaskStoreError::InvalidInput)?;
    if raw.len() > 32768 {
        return Err(TaskStoreError::InvalidInput);
    }
    Ok(raw)
}
