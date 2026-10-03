use super::*;

pub(super) fn event_type(sequence: RevisionNumber) -> &'static str {
    if u64::from(sequence) == 1 {
        "cyanrex.artifact.created"
    } else {
        "cyanrex.artifact.revised"
    }
}
pub(super) fn encode(revision: &ArtifactRevision) -> Result<String> {
    let raw = serde_json::to_string(revision).map_err(|_| ArtifactStoreError::InvalidInput)?;
    if raw.len() > 16384 {
        return Err(ArtifactStoreError::InvalidInput);
    }
    Ok(raw)
}
impl ArtifactStore {
    fn validate_revision(&self, revision: &ArtifactRevision) -> Result<()> {
        self.check_scope(revision.reference.workspace, revision.owner)
            .map_err(|_| ArtifactStoreError::InvalidRecord)?;
        draft::validate_metadata(&revision.title, &revision.media_type, revision.byte_length)
            .map_err(|_| ArtifactStoreError::InvalidRecord)?;
        match (&revision.parent, u64::from(revision.sequence)) {
            (None, 1) => {}
            (Some(parent), sequence)
                if sequence > 1
                    && parent.workspace == self.scope
                    && parent.artifact_id == revision.reference.artifact_id
                    && parent.revision_id != revision.reference.revision_id => {}
            _ => return Err(ArtifactStoreError::InvalidRecord),
        }
        Ok(())
    }
    pub(super) async fn revision(
        &self,
        tx: &mut Tx<'_>,
        artifact: ArtifactId,
        id: ArtifactRevisionId,
        owner: PrincipalRef,
    ) -> Result<Option<ArtifactRevision>> {
        let Some(row) = sqlx::query("SELECT sequence, CASE WHEN octet_length(snapshot) <= 16384 THEN snapshot END AS snapshot
            FROM collaboration_artifact_revisions WHERE artifact_id = $1 AND revision_id = $2")
            .bind(artifact.as_uuid()).bind(id.as_uuid()).fetch_optional(&mut **tx).await? else { return Ok(None); };
        let raw: Option<String> = row.try_get("snapshot")?;
        let revision: ArtifactRevision =
            serde_json::from_str(&raw.ok_or(ArtifactStoreError::InvalidRecord)?)
                .map_err(|_| ArtifactStoreError::InvalidRecord)?;
        self.validate_revision(&revision)?;
        if revision.reference.artifact_id != artifact
            || revision.reference.revision_id != id
            || revision.owner != owner
            || row.try_get::<i64, _>("sequence")? != u64::from(revision.sequence) as i64
        {
            return Err(ArtifactStoreError::InvalidRecord);
        }
        let event = sqlx::query(
            "SELECT event_id, revision_id, actor_id, event_type,
            CASE WHEN octet_length(snapshot) <= 16384 THEN snapshot END AS snapshot
            FROM collaboration_artifact_outbox WHERE artifact_id = $1 AND sequence = $2",
        )
        .bind(artifact.as_uuid())
        .bind(u64::from(revision.sequence) as i64)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(ArtifactStoreError::InvalidRecord)?;
        let raw: Option<String> = event.try_get("snapshot")?;
        let recorded: ArtifactRevision =
            serde_json::from_str(&raw.ok_or(ArtifactStoreError::InvalidRecord)?)
                .map_err(|_| ArtifactStoreError::InvalidRecord)?;
        if recorded != revision
            || event.try_get::<uuid::Uuid, _>("event_id")?.is_nil()
            || event.try_get::<uuid::Uuid, _>("revision_id")? != id.as_uuid()
            || event.try_get::<uuid::Uuid, _>("actor_id")? != owner.principal_id.as_uuid()
            || event.try_get::<String, _>("event_type")? != event_type(revision.sequence)
        {
            return Err(ArtifactStoreError::InvalidRecord);
        }
        Ok(Some(revision))
    }
    pub(super) async fn head(
        &self,
        tx: &mut Tx<'_>,
        artifact: ArtifactId,
        owner: PrincipalRef,
        lock: bool,
    ) -> Result<Option<ArtifactRevision>> {
        let statement = if lock {
            "SELECT head_revision_id, sequence FROM collaboration_artifacts WHERE artifact_id = $1 AND owner_id = $2 FOR UPDATE"
        } else {
            "SELECT head_revision_id, sequence FROM collaboration_artifacts WHERE artifact_id = $1 AND owner_id = $2"
        };
        let Some(row) = sqlx::query(statement)
            .bind(artifact.as_uuid())
            .bind(owner.principal_id.as_uuid())
            .fetch_optional(&mut **tx)
            .await?
        else {
            return Ok(None);
        };
        let id = ArtifactRevisionId::try_from(row.try_get::<uuid::Uuid, _>("head_revision_id")?)
            .map_err(|_| ArtifactStoreError::InvalidRecord)?;
        // Separate statements after the row lock observe a concurrent writer's committed records.
        // Ordinary reads instead use a single repeatable-read database snapshot.
        let revision = self
            .revision(tx, artifact, id, owner)
            .await?
            .ok_or(ArtifactStoreError::InvalidRecord)?;
        if row.try_get::<i64, _>("sequence")? != u64::from(revision.sequence) as i64 {
            return Err(ArtifactStoreError::InvalidRecord);
        }
        // A valid older snapshot is not evidence that the mutable head is current.
        for statement in [
            "SELECT revision_id, sequence FROM collaboration_artifact_revisions WHERE artifact_id = $1 ORDER BY sequence DESC LIMIT 1",
            "SELECT revision_id, sequence FROM collaboration_artifact_outbox WHERE artifact_id = $1 ORDER BY sequence DESC LIMIT 1",
        ] {
            let latest = sqlx::query(statement).bind(artifact.as_uuid()).fetch_optional(&mut **tx).await?
                .ok_or(ArtifactStoreError::InvalidRecord)?;
            if latest.try_get::<uuid::Uuid, _>("revision_id")? != id.as_uuid()
                || latest.try_get::<i64, _>("sequence")? != u64::from(revision.sequence) as i64 {
                return Err(ArtifactStoreError::InvalidRecord);
            }
        }
        Ok(Some(revision))
    }
    pub(super) async fn publish(
        &self,
        tx: &mut Tx<'_>,
        revision: &ArtifactRevision,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let raw = encode(revision)?;
        confirmed_one(sqlx::query("INSERT INTO collaboration_artifact_revisions (revision_id, artifact_id, sequence, snapshot)
            VALUES ($1, $2, $3, $4)")
            .bind(revision.reference.revision_id.as_uuid()).bind(revision.reference.artifact_id.as_uuid())
            .bind(u64::from(revision.sequence) as i64).bind(&raw).execute(&mut **tx).await?.rows_affected())?;
        let (blobs, published) = (self.blobs.clone(), revision.clone());
        tokio::task::spawn_blocking(move || blobs.write_once(&published, &bytes))
            .await
            .map_err(|_| ArtifactStoreError::BlobUnavailable)??;
        confirmed_one(sqlx::query("INSERT INTO collaboration_artifact_outbox (event_id, artifact_id, revision_id, sequence, actor_id, event_type, snapshot)
            VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(uuid::Uuid::new_v4()).bind(revision.reference.artifact_id.as_uuid()).bind(revision.reference.revision_id.as_uuid())
            .bind(u64::from(revision.sequence) as i64).bind(revision.owner.principal_id.as_uuid())
            .bind(event_type(revision.sequence)).bind(raw).execute(&mut **tx).await?.rows_affected())?;
        self.check_schema(tx, true).await?;
        if self
            .head(tx, revision.reference.artifact_id, revision.owner, true)
            .await?
            .as_ref()
            != Some(revision)
        {
            return Err(ArtifactStoreError::InvalidRecord);
        }
        self.read_blob(revision).await?;
        Ok(())
    }
}
