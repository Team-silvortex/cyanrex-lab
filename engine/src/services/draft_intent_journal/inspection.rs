use super::*;

/// Retains the first relations and full immutable read set across a borrowed resource read.
/// Private construction only; never a dispatch permit or a deserialized recovery record.
pub(crate) struct JournalReadSnapshot {
    record: DraftIntentRecord,
    steps: Vec<DraftDispatchRecord>,
    relations: IntentRelations,
}
impl JournalReadSnapshot {
    pub(crate) fn record(&self) -> &DraftIntentRecord {
        &self.record
    }
    pub(crate) fn steps(&self) -> &[DraftDispatchRecord] {
        &self.steps
    }
    pub(crate) async fn verify(&self, journal: &DraftIntentJournal, tx: &mut Tx<'_>) -> Result<()> {
        let last = self.record.validate()?.manifest().payload().len();
        journal
            .verify_records(tx, &self.record, last, &self.relations, &self.steps)
            .await
    }
}

impl DraftIntentJournal {
    /// Borrowed current-Session transaction only. No nonce or owner-only public read surface.
    pub(crate) async fn inspect_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        task: TaskRef,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
    ) -> Result<Option<(DraftIntentRecord, Vec<DraftDispatchRecord>)>> {
        let Some(snapshot) = self
            .capture_in_transaction(tx, task, owner, account_id)
            .await?
        else {
            return Ok(None);
        };
        let JournalReadSnapshot { record, steps, .. } = snapshot;
        Ok(Some((record, steps)))
    }

    pub(crate) async fn capture_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        task: TaskRef,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
    ) -> Result<Option<JournalReadSnapshot>> {
        let relations = self.check_schema(tx).await?;
        // Even a missing intent must not treat schema 1 as a schema-2 empty step list.
        relations.require_dispatch()?;
        let record = self
            .read_in_transaction(tx, task, owner, account_id)
            .await?;
        let Some(record) = record else {
            relations.verify(self, tx).await?;
            return Ok(None);
        };
        let last = record.validate()?.manifest().payload().len();
        let steps = dispatch::read_steps(tx, task, last).await?;
        // A reservation can only be the last row. Never summarize away a malformed suffix.
        if steps
            .iter()
            .take(steps.len().saturating_sub(1))
            .any(|step| !step.committed)
        {
            return Err(DraftIntentJournalError::InvalidRecord);
        }
        self.verify_records(tx, &record, last, &relations, &steps)
            .await?;
        Ok(Some(JournalReadSnapshot {
            record,
            steps,
            relations,
        }))
    }
}
