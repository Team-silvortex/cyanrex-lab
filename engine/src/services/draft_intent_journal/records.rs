use super::*;
use crate::{
    models::collaboration::PrincipalId,
    services::auth_service::durable_source::{
        SessionDraftCheckpointState, SessionDraftPublicationCheckpoint,
        MAX_SESSION_DRAFT_CHECKPOINT_BYTES,
    },
};

impl DraftIntentRecord {
    pub(crate) fn validate(&self) -> Result<SessionDraftPublicationCheckpoint> {
        let checkpoint = SessionDraftPublicationCheckpoint::parse_json(&self.checkpoint)
            .map_err(|_| DraftIntentJournalError::InvalidRecord)?;
        if checkpoint.task_ref() != self.task
            || self.owner.authority_id != self.task.workspace.authority_id
            || checkpoint.reported_state() != SessionDraftCheckpointState::Ready
            || checkpoint.reported_confirmed_artifact_count() != 0
            || checkpoint
                .to_json()
                .map_err(|_| DraftIntentJournalError::InvalidRecord)?
                != self.checkpoint
        {
            return Err(DraftIntentJournalError::InvalidRecord);
        }
        Ok(checkpoint)
    }
}

impl DraftIntentJournal {
    /// Internal current-Session transaction only. No standalone owner-only write/read surface.
    pub(crate) async fn insert_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        record: &DraftIntentRecord,
    ) -> Result<()> {
        if record.task.workspace != self.scope {
            return Err(DraftIntentJournalError::ScopeMismatch);
        }
        record.validate()?;
        let relations = self.check_schema(tx).await?;
        confirmed_one(sqlx::query("INSERT INTO collaboration_draft_intents
            (task_id, owner_id, account_id, source_namespace, task_namespace, artifact_namespace, checkpoint)
            VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .bind(record.task.task_id.as_uuid()).bind(record.owner.principal_id.as_uuid())
            .bind(record.account_id.as_uuid()).bind(&record.source_namespace)
            .bind(&record.task_namespace).bind(&record.artifact_namespace).bind(&record.checkpoint)
            .execute(&mut **tx).await?.rows_affected())?;
        let stored = self
            .read_in_transaction(tx, record.task, record.owner, record.account_id)
            .await?
            .ok_or(DraftIntentJournalError::InvalidRecord)?;
        if stored != *record {
            return Err(DraftIntentJournalError::InvalidRecord);
        }
        // Trigger-induced version/installation/scope drift must not confirm the operation.
        relations.verify(self, tx).await?;
        Ok(())
    }

    pub(crate) async fn read_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        task: TaskRef,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
    ) -> Result<Option<DraftIntentRecord>> {
        if task.workspace != self.scope || owner.authority_id != self.scope.authority_id {
            return Err(DraftIntentJournalError::ScopeMismatch);
        }
        let relations = self.check_schema(tx).await?;
        // Bound before copying BYTEA from the database. Corruption is not a missing intent.
        let row = sqlx::query("SELECT task_id, owner_id, account_id,
            CASE WHEN octet_length(source_namespace) <= 63 THEN source_namespace ELSE NULL END AS source_namespace,
            CASE WHEN octet_length(task_namespace) <= 63 THEN task_namespace ELSE NULL END AS task_namespace,
            CASE WHEN octet_length(artifact_namespace) <= 63 THEN artifact_namespace ELSE NULL END AS artifact_namespace,
            CASE WHEN octet_length(checkpoint) <= $4 THEN checkpoint ELSE NULL END AS checkpoint
            FROM collaboration_draft_intents WHERE task_id = $1 AND owner_id = $2 AND account_id = $3 FOR SHARE")
            .bind(task.task_id.as_uuid()).bind(owner.principal_id.as_uuid()).bind(account_id.as_uuid())
            .bind(MAX_SESSION_DRAFT_CHECKPOINT_BYTES as i32).fetch_optional(&mut **tx).await?;
        relations.verify(self, tx).await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let record = DraftIntentRecord {
            task: TaskRef {
                workspace: self.scope,
                task_id: row
                    .try_get::<Uuid, _>("task_id")?
                    .try_into()
                    .map_err(|_| DraftIntentJournalError::InvalidRecord)?,
            },
            owner: PrincipalRef {
                authority_id: self.scope.authority_id,
                principal_id: PrincipalId::try_from(row.try_get::<Uuid, _>("owner_id")?)
                    .map_err(|_| DraftIntentJournalError::InvalidRecord)?,
            },
            account_id: row
                .try_get::<Uuid, _>("account_id")?
                .try_into()
                .map_err(|_| DraftIntentJournalError::InvalidRecord)?,
            source_namespace: bounded_namespace(&row, "source_namespace")?,
            task_namespace: bounded_namespace(&row, "task_namespace")?,
            artifact_namespace: bounded_namespace(&row, "artifact_namespace")?,
            checkpoint: row
                .try_get::<Option<Vec<u8>>, _>("checkpoint")?
                .ok_or(DraftIntentJournalError::InvalidRecord)?,
        };
        record.validate()?;
        Ok(Some(record))
    }
}

fn bounded_namespace(row: &sqlx::postgres::PgRow, name: &str) -> Result<String> {
    row.try_get::<Option<String>, _>(name)?
        .ok_or(DraftIntentJournalError::InvalidRecord)
}
