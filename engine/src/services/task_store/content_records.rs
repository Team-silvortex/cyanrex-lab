use super::*;

pub(super) fn event_type(snapshot: &TaskSnapshot) -> &'static str {
    // Schema 3 exposes only whole-content edits in Draft; legacy input replacement is excluded.
    if u64::from(snapshot.revision) == 1 {
        "cyanrex.task.created"
    } else if snapshot.status == TaskStatus::Draft {
        "cyanrex.task.content_updated"
    } else {
        "cyanrex.task.status_changed"
    }
}

pub(super) fn encode(snapshot: &TaskContentSnapshot) -> Result<(String, String)> {
    let task = task_records::encode(&snapshot.task)?;
    let manifest =
        serde_json::to_string(&snapshot.manifest).map_err(|_| TaskStoreError::InvalidInput)?;
    if manifest.len() > MAX_TASK_CONTENT_MANIFEST_BYTES {
        return Err(TaskStoreError::InvalidInput);
    }
    Ok((task, manifest))
}

impl TaskContentStore {
    fn decode(
        &self,
        task: Option<String>,
        manifest: Option<String>,
    ) -> Result<TaskContentSnapshot> {
        // Neither SQL NULL nor an over-limit projection means "metadata absent" in schema 3.
        let task = serde_json::from_str(&task.ok_or(TaskStoreError::InvalidRecord)?)
            .map_err(|_| TaskStoreError::InvalidRecord)?;
        let manifest = TaskContentManifest::parse_json(
            manifest.ok_or(TaskStoreError::InvalidRecord)?.as_bytes(),
        )
        .map_err(|_| TaskStoreError::InvalidRecord)?;
        let snapshot = TaskContentSnapshot { task, manifest };
        self.validate_record(&snapshot)?;
        Ok(snapshot)
    }

    pub(super) async fn read(
        &self,
        tx: &mut Tx<'_>,
        reference: TaskRef,
        owner: PrincipalRef,
        lock: bool,
    ) -> Result<Option<TaskContentSnapshot>> {
        let statement = if lock {
            "SELECT owner_id, revision, CASE WHEN octet_length(snapshot) <= 32768 THEN snapshot END AS snapshot,
            CASE WHEN octet_length(manifest) <= 65536 THEN manifest END AS manifest
            FROM collaboration_tasks WHERE task_id = $1 AND owner_id = $2 FOR UPDATE"
        } else {
            "SELECT owner_id, revision, CASE WHEN octet_length(snapshot) <= 32768 THEN snapshot END AS snapshot,
            CASE WHEN octet_length(manifest) <= 65536 THEN manifest END AS manifest
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
        let snapshot = self.decode(row.try_get("snapshot")?, row.try_get("manifest")?)?;
        if snapshot.task.reference != reference
            || snapshot.task.owner != owner
            || row.try_get::<uuid::Uuid, _>("owner_id")? != owner.principal_id.as_uuid()
            || row.try_get::<i64, _>("revision")? != u64::from(snapshot.task.revision) as i64
        {
            return Err(TaskStoreError::InvalidRecord);
        }
        // A new statement after FOR UPDATE sees the winner's committed event. Ordinary reads
        // instead run under the shared repeatable-read transaction, so the pair cannot tear.
        let event = sqlx::query(
            "SELECT event_id, actor_id, revision, event_type,
            CASE WHEN octet_length(snapshot) <= 32768 THEN snapshot END AS snapshot,
            CASE WHEN octet_length(manifest) <= 65536 THEN manifest END AS manifest
            FROM collaboration_task_outbox WHERE task_id = $1 ORDER BY revision DESC LIMIT 1",
        )
        .bind(reference.task_id.as_uuid())
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(TaskStoreError::InvalidRecord)?;
        let recorded = self.decode(event.try_get("snapshot")?, event.try_get("manifest")?)?;
        if recorded != snapshot
            || event.try_get::<uuid::Uuid, _>("event_id")?.is_nil()
            || event.try_get::<uuid::Uuid, _>("actor_id")? != owner.principal_id.as_uuid()
            || event.try_get::<i64, _>("revision")? != u64::from(snapshot.task.revision) as i64
            || event.try_get::<String, _>("event_type")? != event_type(&snapshot.task)
        {
            return Err(TaskStoreError::InvalidRecord);
        }
        Ok(Some(snapshot))
    }

    pub(super) async fn update_and_confirm(
        &self,
        tx: &mut Tx<'_>,
        snapshot: &TaskContentSnapshot,
        expected_revision: RevisionNumber,
    ) -> Result<()> {
        self.validate_record(snapshot)?;
        let (raw, metadata) = encode(snapshot)?;
        confirmed_one(
            sqlx::query(
                "UPDATE collaboration_tasks SET revision = $3, snapshot = $4, manifest = $5
            WHERE task_id = $1 AND owner_id = $2 AND revision = $6",
            )
            .bind(snapshot.task.reference.task_id.as_uuid())
            .bind(snapshot.task.owner.principal_id.as_uuid())
            .bind(u64::from(snapshot.task.revision) as i64)
            .bind(&raw)
            .bind(&metadata)
            .bind(u64::from(expected_revision) as i64)
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )?;
        self.append_and_confirm(tx, snapshot, &raw, &metadata).await
    }

    pub(super) async fn append_and_confirm(
        &self,
        tx: &mut Tx<'_>,
        snapshot: &TaskContentSnapshot,
        raw: &str,
        manifest: &str,
    ) -> Result<()> {
        confirmed_one(
            sqlx::query(
                "INSERT INTO collaboration_task_outbox
            (event_id, task_id, revision, actor_id, event_type, snapshot, manifest)
            VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(uuid::Uuid::new_v4())
            .bind(snapshot.task.reference.task_id.as_uuid())
            .bind(u64::from(snapshot.task.revision) as i64)
            .bind(snapshot.task.owner.principal_id.as_uuid())
            .bind(event_type(&snapshot.task))
            .bind(raw)
            .bind(manifest)
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )?;
        self.store.check_schema(tx, true).await?;
        // Head/event agreement alone misses a trigger rewriting both consistently. Compare the
        // entire pair with the expected operation, not merely its references or current revision.
        if self
            .read(tx, snapshot.task.reference, snapshot.task.owner, true)
            .await?
            .as_ref()
            != Some(snapshot)
        {
            return Err(TaskStoreError::InvalidRecord);
        }
        Ok(())
    }
}
