use super::*;

const MAX_STEPS: usize = 33;

impl DraftIntentJournal {
    /// Pending reservation only. A confirmed outer COMMIT is required before any resource work.
    /// Every occupied ordinal conflicts, including a matching unknown nonce.
    pub(crate) async fn reserve_step_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        record: &DraftIntentRecord,
        ordinal: usize,
        nonce: Uuid,
    ) -> Result<()> {
        let last = self.step_limit(record, ordinal, nonce)?;
        let relations = self.check_schema(tx).await?;
        relations.require_dispatch()?;
        self.require_intent(tx, record).await?;
        let mut expected = read_steps(tx, record.task, last).await?;
        require_prefix(&expected, ordinal)?;
        if expected.len() != ordinal {
            return Err(DraftIntentJournalError::Conflict);
        }
        expected.push(DraftDispatchRecord {
            task: record.task,
            ordinal,
            nonce,
            committed: false,
        });
        confirmed_one(
            sqlx::query(
                "INSERT INTO collaboration_draft_dispatch_steps
            (task_id, ordinal, nonce, committed) VALUES ($1, $2, $3, FALSE)",
            )
            .bind(record.task.task_id.as_uuid())
            .bind(ordinal as i32)
            .bind(nonce)
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )?;
        self.verify_records(tx, record, last, &relations, &expected)
            .await
    }

    /// Marks this single reserved step in the same transaction that will create the resource.
    /// It remains Unknown outside this transaction until the caller confirms the business COMMIT.
    pub(crate) async fn begin_step_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        record: &DraftIntentRecord,
        ordinal: usize,
        nonce: Uuid,
    ) -> Result<IntentRelations> {
        let last = self.step_limit(record, ordinal, nonce)?;
        let mut relations = self.check_schema(tx).await?;
        relations.require_dispatch()?;
        self.require_intent(tx, record).await?;
        let mut expected = read_steps(tx, record.task, last).await?;
        require_prefix(&expected, ordinal)?;
        if expected.len() != ordinal + 1
            || expected[ordinal].nonce != nonce
            || expected[ordinal].committed
        {
            return Err(DraftIntentJournalError::Conflict);
        }
        expected[ordinal].committed = true;
        confirmed_one(
            sqlx::query(
                "UPDATE collaboration_draft_dispatch_steps SET committed = TRUE
            WHERE task_id = $1 AND ordinal = $2 AND nonce = $3 AND NOT committed",
            )
            .bind(record.task.task_id.as_uuid())
            .bind(ordinal as i32)
            .bind(nonce)
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )?;
        self.verify_records(tx, record, last, &relations, &expected)
            .await?;
        // Later resource triggers must not rewrite even the preceding confirmed nonces.
        relations.expected_steps = Some(expected);
        Ok(relations)
    }

    /// Read-only revalidation after ALL business writes. Never completes or repairs a step here.
    pub(crate) async fn verify_step_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        record: &DraftIntentRecord,
        ordinal: usize,
        nonce: Uuid,
        relations: &IntentRelations,
    ) -> Result<()> {
        let last = self.step_limit(record, ordinal, nonce)?;
        relations.require_dispatch()?;
        let expected = relations
            .expected_steps
            .as_ref()
            .ok_or(DraftIntentJournalError::InvalidInput)?;
        if expected.len() != ordinal + 1
            || expected[ordinal].task != record.task
            || expected[ordinal].nonce != nonce
            || !expected[ordinal].committed
        {
            return Err(DraftIntentJournalError::InvalidInput);
        }
        self.verify_records(tx, record, last, relations, expected)
            .await
    }

    fn step_limit(&self, record: &DraftIntentRecord, ordinal: usize, nonce: Uuid) -> Result<usize> {
        if record.task.workspace != self.scope
            || record.owner.authority_id != self.scope.authority_id
        {
            return Err(DraftIntentJournalError::ScopeMismatch);
        }
        let last = record.validate()?.manifest().payload().len();
        if ordinal > last || ordinal >= MAX_STEPS || !valid_nonce(nonce) {
            return Err(DraftIntentJournalError::InvalidInput);
        }
        Ok(last)
    }

    async fn require_intent(&self, tx: &mut Tx<'_>, expected: &DraftIntentRecord) -> Result<()> {
        let actual = self
            .read_in_transaction(tx, expected.task, expected.owner, expected.account_id)
            .await?
            .ok_or(DraftIntentJournalError::InvalidRecord)?;
        if actual != *expected {
            return Err(DraftIntentJournalError::InvalidRecord);
        }
        Ok(())
    }

    pub(super) async fn verify_records(
        &self,
        tx: &mut Tx<'_>,
        record: &DraftIntentRecord,
        last: usize,
        relations: &IntentRelations,
        expected: &[DraftDispatchRecord],
    ) -> Result<()> {
        relations.verify(self, tx).await?;
        self.require_intent(tx, record).await?;
        if read_steps(tx, record.task, last).await? != expected {
            return Err(DraftIntentJournalError::InvalidRecord);
        }
        relations.verify(self, tx).await
    }
}

fn valid_nonce(nonce: Uuid) -> bool {
    nonce.get_version() == Some(uuid::Version::Random)
        && nonce.get_variant() == uuid::Variant::RFC4122
}

fn require_prefix(steps: &[DraftDispatchRecord], ordinal: usize) -> Result<()> {
    if steps.len() < ordinal {
        return Err(DraftIntentJournalError::InvalidRecord);
    }
    if steps.iter().take(ordinal).any(|step| !step.committed) {
        return Err(DraftIntentJournalError::Conflict);
    }
    Ok(())
}

pub(super) async fn read_steps(
    tx: &mut Tx<'_>,
    task: TaskRef,
    last: usize,
) -> Result<Vec<DraftDispatchRecord>> {
    // The extra row detects overflow instead of silently accepting a truncated prefix.
    let rows = sqlx::query(
        "SELECT task_id, ordinal, nonce, committed
        FROM collaboration_draft_dispatch_steps WHERE task_id = $1
        ORDER BY ordinal LIMIT 34 FOR UPDATE",
    )
    .bind(task.task_id.as_uuid())
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() > MAX_STEPS || rows.len() > last + 1 {
        return Err(DraftIntentJournalError::InvalidRecord);
    }
    let mut steps: Vec<DraftDispatchRecord> = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        let ordinal: i32 = row.try_get("ordinal")?;
        let nonce: Uuid = row.try_get("nonce")?;
        if row.try_get::<Uuid, _>("task_id")? != task.task_id.as_uuid()
            || ordinal < 0
            || ordinal as usize != index
            || index > last
            || !valid_nonce(nonce)
            || steps.iter().any(|step| step.nonce == nonce)
        {
            return Err(DraftIntentJournalError::InvalidRecord);
        }
        steps.push(DraftDispatchRecord {
            task,
            ordinal: index,
            nonce,
            committed: row.try_get("committed")?,
        });
    }
    Ok(steps)
}
