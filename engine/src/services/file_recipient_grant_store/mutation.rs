//! Caller-owned transaction mutations. Pending metadata is not an acknowledged permission.
use super::*;
use ledger::Ledger;
use records::{canonical, Change, Selected};
use schema::SchemaPin;

/// Move-only original schema, full selected history and exact expected whole-ledger delta.
/// The caller flushes deferred work, verifies this object, finishes current authority checks
/// and confirms COMMIT before consuming the record. No issuer authentication occurs here.
pub(crate) struct PendingFileRecipientGrantMutation {
    schema: SchemaPin,
    selected: Selected,
    fingerprint: [u8; 32],
    ledger: Ledger,
}

impl PendingFileRecipientGrantMutation {
    pub(crate) fn into_record(self) -> StoredFileRecipientGrant {
        self.selected.record
    }
}

impl FileRecipientGrantStore {
    pub(crate) async fn issue_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        grant_id: FileGrantId,
        issuer: FileAccountRef,
        file: FileVersionRef,
        publisher: FileAccountRef,
        recipient: FileAccountRef,
        validity: FileGrantValidity,
    ) -> Result<PendingFileRecipientGrantMutation> {
        self.check_account(issuer)?;
        self.check_account(publisher)?;
        self.check_account(recipient)?;
        if file.workspace() != self.scope {
            return Err(FileRecipientGrantStoreError::ScopeMismatch);
        }
        let grant = FileRecipientGrant::new(
            grant_id,
            RevisionNumber::try_from(1).map_err(|_| FileRecipientGrantStoreError::InvalidInput)?,
            file,
            publisher,
            recipient,
            validity,
            FileGrantState::Active,
        )
        .map_err(|_| FileRecipientGrantStoreError::InvalidInput)?;
        let schema = self.check_schema(tx, true).await?;
        let mut expected = self.ledger(tx).await?;
        if expected.contains(grant_id) {
            return Err(FileRecipientGrantStoreError::Conflict);
        }
        if expected.len() >= self.max_grants as usize {
            return Err(FileRecipientGrantStoreError::QuotaExceeded);
        }
        let change = Change {
            grant: grant.clone(),
            actor: issuer,
            recorded_at: self.database_now(tx).await?,
        };
        let selected = Selected {
            record: StoredFileRecipientGrant { grant, issuer },
            changes: vec![change],
        };
        let fingerprint = selected.fingerprint()?;
        expected.add(&selected)?;
        confirmed_one(
            sqlx::query(
                "INSERT INTO collaboration_file_recipient_grants
            (grant_id,recipient_id,recipient_account_id,publisher_id,publisher_account_id,
            issuer_id,issuer_account_id,revision,not_before,expires_at,grant_payload)
            VALUES($1,$2,$3,$4,$5,$6,$7,1,$8,$9,$10)",
            )
            .bind(grant_id.as_uuid())
            .bind(recipient.principal().principal_id.as_uuid())
            .bind(recipient.account_id().as_uuid())
            .bind(publisher.principal().principal_id.as_uuid())
            .bind(publisher.account_id().as_uuid())
            .bind(issuer.principal().principal_id.as_uuid())
            .bind(issuer.account_id().as_uuid())
            .bind(validity.not_before() as i64)
            .bind(validity.expires_at() as i64)
            .bind(canonical(&selected.record.grant)?)
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )?;
        // Preserve the first identity before another unqualified write, including trigger drift.
        schema.verify(self, tx, true).await?;
        self.insert_change(tx, &selected.changes[0]).await?;
        Ok(PendingFileRecipientGrantMutation {
            schema,
            selected,
            fingerprint,
            ledger: expected,
        })
    }

    /// Match the exact recorded publisher before any mutation, under the original writer fence.
    /// None hides a missing/foreign coordinate; it is not a permission or a whole-ledger receipt.
    /// First issuer attribution may differ from publisher and is retained unchanged.
    #[cfg(unix)]
    pub(crate) async fn revoke_owned_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        grant_id: FileGrantId,
        actor: FileAccountRef,
        expected_revision: RevisionNumber,
    ) -> Result<Option<PendingFileRecipientGrantMutation>> {
        self.revoke_pending_in_transaction(tx, grant_id, actor, expected_revision, true)
            .await
    }

    pub(super) async fn revoke_trusted_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        grant_id: FileGrantId,
        actor: FileAccountRef,
        expected_revision: RevisionNumber,
    ) -> Result<PendingFileRecipientGrantMutation> {
        self.revoke_pending_in_transaction(tx, grant_id, actor, expected_revision, false)
            .await?
            .ok_or(FileRecipientGrantStoreError::Conflict)
    }

    async fn revoke_pending_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        grant_id: FileGrantId,
        actor: FileAccountRef,
        expected_revision: RevisionNumber,
        owned: bool,
    ) -> Result<Option<PendingFileRecipientGrantMutation>> {
        self.check_account(actor)?;
        // Acquire SRE directly. Taking the read helper's SHARE fence first would create a
        // same-ledger lock-upgrade cycle for concurrent mutations.
        let schema = self.check_schema(tx, true).await?;
        let mut expected = self.ledger(tx).await?;
        let Some(mut selected) = self.read_selected(tx, grant_id, None, false).await? else {
            return if owned {
                Ok(None)
            } else {
                Err(FileRecipientGrantStoreError::Conflict)
            };
        };
        let previous = &selected.record.grant;
        if owned && previous.publisher() != actor {
            return Ok(None);
        }
        if u64::from(expected_revision) != 1
            || previous.revision() != expected_revision
            || previous.state() != FileGrantState::Active
        {
            return Err(FileRecipientGrantStoreError::Conflict);
        }
        let grant = FileRecipientGrant::new(
            grant_id,
            RevisionNumber::try_from(2).map_err(|_| FileRecipientGrantStoreError::InvalidInput)?,
            previous.file().clone(),
            previous.publisher(),
            previous.recipient(),
            previous.validity(),
            FileGrantState::Revoked,
        )
        .map_err(|_| FileRecipientGrantStoreError::InvalidInput)?;
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
                "UPDATE collaboration_file_recipient_grants
            SET revision=2,grant_payload=$2 WHERE grant_id=$1 AND revision=1",
            )
            .bind(grant_id.as_uuid())
            .bind(raw)
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )?;
        schema.verify(self, tx, true).await?;
        self.insert_change(tx, &selected.changes[1]).await?;
        Ok(Some(PendingFileRecipientGrantMutation {
            schema,
            selected,
            fingerprint,
            ledger: expected,
        }))
    }

    pub(crate) async fn verify_mutation_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFileRecipientGrantMutation,
    ) -> Result<()> {
        // The caller flushes deferred work first. Never begin/commit, change settings,
        // recapture a replacement installation or inspect only the public grant projection.
        pending.schema.verify(self, tx, true).await?;
        let current = self
            .read_selected(tx, pending.selected.record.grant.id(), None, false)
            .await?
            .ok_or(FileRecipientGrantStoreError::InvalidRecord)?;
        if current != pending.selected
            || current.fingerprint()? != pending.fingerprint
            || self.ledger(tx).await? != pending.ledger
        {
            return Err(FileRecipientGrantStoreError::InvalidRecord);
        }
        pending.schema.verify(self, tx, true).await?;
        Ok(())
    }
}
