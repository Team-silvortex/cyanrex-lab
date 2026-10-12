//! Caller-owned transaction mutation; pending data is not an acknowledged permission receipt.
use super::*;
use ledger::Ledger;
use records::{canonical, Change, Selected};
use schema::SchemaPin;

/// Move-only pending record. The caller must recheck original pins and the complete expected
/// ledger after flushing deferred constraints, finish authority checks and confirm COMMIT first.
pub(crate) struct PendingFilePublisherGrantMutation {
    schema: SchemaPin,
    selected: Selected,
    fingerprint: [u8; 32],
    ledger: Ledger,
}

impl PendingFilePublisherGrantMutation {
    pub(crate) fn into_record(self) -> StoredFilePublisherGrant {
        self.selected.record
    }
}

impl FilePublisherGrantStore {
    pub(crate) async fn issue_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        grant_id: FileGrantId,
        issuer: FileAccountRef,
        subject: FileAccountRef,
        validity: FileGrantValidity,
    ) -> Result<PendingFilePublisherGrantMutation> {
        self.check_account(issuer)?;
        self.check_account(subject)?;
        let grant = FilePublisherGrant::new(
            grant_id,
            RevisionNumber::try_from(1).map_err(|_| FilePublisherGrantStoreError::InvalidInput)?,
            self.scope,
            subject,
            validity,
            FileGrantState::Active,
        )
        .map_err(|_| FilePublisherGrantStoreError::InvalidInput)?;
        let schema = self.check_schema(tx, true).await?;
        let mut expected = self.ledger(tx).await?;
        if expected.contains(grant_id) {
            return Err(FilePublisherGrantStoreError::Conflict);
        }
        if expected.len() >= self.max_grants as usize {
            return Err(FilePublisherGrantStoreError::QuotaExceeded);
        }
        let change = Change {
            grant: grant.clone(),
            actor: issuer,
            recorded_at: self.database_now(tx).await?,
        };
        let selected = Selected {
            record: StoredFilePublisherGrant { grant, issuer },
            changes: vec![change],
        };
        let fingerprint = selected.fingerprint()?;
        expected.add(&selected)?;
        confirmed_one(sqlx::query("INSERT INTO collaboration_file_publisher_grants
            (grant_id,subject_id,subject_account_id,issuer_id,issuer_account_id,revision,not_before,expires_at,grant_payload)
            VALUES($1,$2,$3,$4,$5,1,$6,$7,$8)")
            .bind(grant_id.as_uuid()).bind(subject.principal().principal_id.as_uuid()).bind(subject.account_id().as_uuid())
            .bind(issuer.principal().principal_id.as_uuid()).bind(issuer.account_id().as_uuid())
            .bind(validity.not_before() as i64).bind(validity.expires_at() as i64).bind(canonical(&selected.record.grant)?)
            .execute(&mut **tx).await?.rows_affected())?;
        // Preserve the first identity before the next unqualified write, including trigger drift.
        schema.verify(self, tx, true).await?;
        self.insert_change(tx, &selected.changes[0]).await?;
        Ok(PendingFilePublisherGrantMutation {
            schema,
            selected,
            fingerprint,
            ledger: expected,
        })
    }

    pub(crate) async fn revoke_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        grant_id: FileGrantId,
        actor: FileAccountRef,
        expected_revision: RevisionNumber,
    ) -> Result<PendingFilePublisherGrantMutation> {
        self.check_account(actor)?;
        let schema = self.check_schema(tx, true).await?;
        let mut expected = self.ledger(tx).await?;
        let mut selected = self
            .read_selected(tx, grant_id, None, false)
            .await?
            .ok_or(FilePublisherGrantStoreError::Conflict)?;
        let previous = &selected.record.grant;
        if u64::from(expected_revision) != 1
            || previous.revision() != expected_revision
            || previous.state() != FileGrantState::Active
        {
            return Err(FilePublisherGrantStoreError::Conflict);
        }
        let grant = FilePublisherGrant::new(
            grant_id,
            RevisionNumber::try_from(2).map_err(|_| FilePublisherGrantStoreError::InvalidInput)?,
            self.scope,
            previous.subject(),
            previous.validity(),
            FileGrantState::Revoked,
        )
        .map_err(|_| FilePublisherGrantStoreError::InvalidInput)?;
        let change = Change {
            grant: grant.clone(),
            actor,
            recorded_at: self.database_now(tx).await?,
        };
        let raw = canonical(&grant)?;
        selected.record.grant = grant;
        selected.changes.push(change);
        let fingerprint = selected.fingerprint()?;
        expected.replace(&selected)?;
        confirmed_one(
            sqlx::query(
                "UPDATE collaboration_file_publisher_grants SET revision=2,grant_payload=$2
            WHERE grant_id=$1 AND revision=1",
            )
            .bind(grant_id.as_uuid())
            .bind(raw)
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )?;
        schema.verify(self, tx, true).await?;
        self.insert_change(tx, &selected.changes[1]).await?;
        Ok(PendingFilePublisherGrantMutation {
            schema,
            selected,
            fingerprint,
            ledger: expected,
        })
    }

    pub(crate) async fn verify_mutation_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFilePublisherGrantMutation,
    ) -> Result<()> {
        // Caller flushes deferred constraint work first. Never begin/commit, change settings or
        // recapture a replacement installation here; the original write fence remains held.
        pending.schema.verify(self, tx, true).await?;
        let current = self
            .read_selected(tx, pending.selected.record.grant.id(), None, false)
            .await?
            .ok_or(FilePublisherGrantStoreError::InvalidRecord)?;
        if current != pending.selected
            || current.fingerprint()? != pending.fingerprint
            || self.ledger(tx).await? != pending.ledger
        {
            return Err(FilePublisherGrantStoreError::InvalidRecord);
        }
        pending.schema.verify(self, tx, true).await?;
        Ok(())
    }
}
