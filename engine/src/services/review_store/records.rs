use super::*;

pub(super) fn event_type(revision: RevisionNumber) -> &'static str {
    if u64::from(revision) == 1 {
        "cyanrex.review.recorded"
    } else {
        "cyanrex.review.amended"
    }
}
pub(super) fn encode(record: &ReviewRecord) -> Result<String> {
    let raw = serde_json::to_string(record).map_err(|_| ReviewStoreError::InvalidInput)?;
    if raw.len() > 65536 {
        return Err(ReviewStoreError::InvalidInput);
    }
    Ok(raw)
}
fn decode(raw: Option<String>) -> Result<ReviewRecord> {
    serde_json::from_str(&raw.ok_or(ReviewStoreError::InvalidRecord)?)
        .map_err(|_| ReviewStoreError::InvalidRecord)
}
pub(super) fn same_subject(a: &ReviewRecord, b: &ReviewRecord) -> bool {
    a.reference.workspace == b.reference.workspace
        && a.reference.review_id == b.reference.review_id
        && a.reviewer == b.reviewer
        && a.targets == b.targets
        && a.evidence == b.evidence
        && match (&a.judgment, &b.judgment) {
            (ReviewJudgment::Human { policy: a, .. }, ReviewJudgment::Human { policy: b, .. }) => {
                a == b
            }
            (ReviewJudgment::Rule { assessment: a }, ReviewJudgment::Rule { assessment: b }) => {
                a == b
            }
            _ => false,
        }
}
impl ReviewStore {
    pub(super) fn validate_record(&self, record: &ReviewRecord) -> Result<()> {
        self.check_scope(record.reference.workspace, record.reviewer)
            .map_err(|_| ReviewStoreError::InvalidRecord)?;
        draft::validate(&record.targets, &record.evidence, &record.judgment)
            .map_err(|_| ReviewStoreError::InvalidRecord)?;
        if record
            .targets
            .iter()
            .chain(&record.evidence)
            .any(|reference| reference.workspace != self.scope)
        {
            return Err(ReviewStoreError::InvalidRecord);
        }
        let revision = u64::from(record.reference.revision);
        match record.supersedes {
            None if revision == 1 => {}
            Some(previous)
                if revision > 1
                    && previous.workspace == self.scope
                    && previous.review_id == record.reference.review_id
                    && u64::from(previous.revision) == revision - 1
                    && matches!(record.judgment, ReviewJudgment::Human { .. }) => {}
            _ => return Err(ReviewStoreError::InvalidRecord),
        }
        Ok(())
    }
    pub(super) async fn record(
        &self,
        tx: &mut Tx<'_>,
        reference: ReviewRef,
        reviewer: PrincipalRef,
    ) -> Result<ReviewRecord> {
        let row = sqlx::query(
            "SELECT CASE WHEN octet_length(snapshot) <= 65536 THEN snapshot END AS snapshot
            FROM collaboration_review_revisions WHERE review_id = $1 AND revision = $2",
        )
        .bind(reference.review_id.as_uuid())
        .bind(u64::from(reference.revision) as i64)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(ReviewStoreError::InvalidRecord)?;
        let record = decode(row.try_get("snapshot")?)?;
        self.validate_record(&record)?;
        if record.reference != reference || record.reviewer != reviewer {
            return Err(ReviewStoreError::InvalidRecord);
        }
        let event = sqlx::query(
            "SELECT event_id, actor_id, event_type,
            CASE WHEN octet_length(snapshot) <= 65536 THEN snapshot END AS snapshot
            FROM collaboration_review_outbox WHERE review_id = $1 AND revision = $2",
        )
        .bind(reference.review_id.as_uuid())
        .bind(u64::from(reference.revision) as i64)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(ReviewStoreError::InvalidRecord)?;
        if decode(event.try_get("snapshot")?)? != record
            || event.try_get::<uuid::Uuid, _>("event_id")?.is_nil()
            || event.try_get::<uuid::Uuid, _>("actor_id")? != reviewer.principal_id.as_uuid()
            || event.try_get::<String, _>("event_type")? != event_type(reference.revision)
        {
            return Err(ReviewStoreError::InvalidRecord);
        }
        Ok(record)
    }
    pub(super) async fn head(
        &self,
        tx: &mut Tx<'_>,
        review_id: ReviewId,
        reviewer: PrincipalRef,
        lock: bool,
    ) -> Result<Option<ReviewRecord>> {
        let statement = if lock {
            "SELECT revision FROM collaboration_reviews WHERE review_id = $1 AND reviewer_id = $2 FOR UPDATE"
        } else {
            "SELECT revision FROM collaboration_reviews WHERE review_id = $1 AND reviewer_id = $2"
        };
        let Some(row) = sqlx::query(statement)
            .bind(review_id.as_uuid())
            .bind(reviewer.principal_id.as_uuid())
            .fetch_optional(&mut **tx)
            .await?
        else {
            return Ok(None);
        };
        let revision: i64 = row.try_get("revision")?;
        let reference = ReviewRef {
            workspace: self.scope,
            review_id,
            revision: (revision as u64)
                .try_into()
                .map_err(|_| ReviewStoreError::InvalidRecord)?,
        };
        // Separate post-lock statements see a waiting writer's committed winner under READ COMMITTED.
        // Read-only operations instead keep one repeatable-read snapshot throughout.
        let latest = sqlx::query("SELECT
            (SELECT revision FROM collaboration_review_revisions WHERE review_id = $1 ORDER BY revision DESC LIMIT 1) AS record,
            (SELECT revision FROM collaboration_review_outbox WHERE review_id = $1 ORDER BY revision DESC LIMIT 1) AS event")
            .bind(review_id.as_uuid()).fetch_one(&mut **tx).await?;
        if latest.try_get::<Option<i64>, _>("record")? != Some(revision)
            || latest.try_get::<Option<i64>, _>("event")? != Some(revision)
        {
            return Err(ReviewStoreError::InvalidRecord);
        }
        Ok(Some(self.record(tx, reference, reviewer).await?))
    }
    pub(super) async fn append_and_confirm(
        &self,
        tx: &mut Tx<'_>,
        record: &ReviewRecord,
        raw: &str,
    ) -> Result<()> {
        confirmed_one(sqlx::query("INSERT INTO collaboration_review_revisions (review_id, revision, snapshot) VALUES ($1, $2, $3)")
            .bind(record.reference.review_id.as_uuid()).bind(u64::from(record.reference.revision) as i64).bind(raw)
            .execute(&mut **tx).await?.rows_affected())?;
        confirmed_one(sqlx::query("INSERT INTO collaboration_review_outbox (event_id, review_id, revision, actor_id, event_type, snapshot)
            VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(uuid::Uuid::new_v4()).bind(record.reference.review_id.as_uuid()).bind(u64::from(record.reference.revision) as i64)
            .bind(record.reviewer.principal_id.as_uuid()).bind(event_type(record.reference.revision)).bind(raw)
            .execute(&mut **tx).await?.rows_affected())?;
        self.check_schema(tx, true).await?;
        if self
            .head(tx, record.reference.review_id, record.reviewer, true)
            .await?
            .as_ref()
            != Some(record)
        {
            return Err(ReviewStoreError::InvalidRecord);
        }
        Ok(())
    }
}
