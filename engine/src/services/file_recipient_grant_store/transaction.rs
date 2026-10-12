//! Borrowed own-recipient metadata; no independent pool, authority check, settings or COMMIT.
use super::*;
use records::Selected;
use schema::SchemaPin;

/// Original schema/head/history snapshot. The caller verifies the selected target, finishes
/// current authority checks and confirms COMMIT before consuming this private pending metadata.
pub(crate) struct PendingFileRecipientGrantRead {
    schema: SchemaPin,
    grant_id: FileGrantId,
    recipient: FileAccountRef,
    expected: Option<(Selected, [u8; 32])>,
    share: bool,
}

impl PendingFileRecipientGrantRead {
    /// Borrow the exact selected history's record without dropping its original pins or
    /// manufacturing current authority. The source must still verify and confirm COMMIT.
    #[cfg(unix)]
    pub(crate) fn record(&self) -> Option<&StoredFileRecipientGrant> {
        self.expected.as_ref().map(|value| &value.0.record)
    }

    pub(crate) fn into_record(self) -> Option<StoredFileRecipientGrant> {
        self.expected.map(|value| value.0.record)
    }
}

impl FileRecipientGrantStore {
    /// Reacquire the ORIGINAL selected history's fences after a caller-owned file-work gap.
    /// No new installation or selected grant is captured, and no transaction/settings are owned.
    #[cfg(unix)]
    pub(crate) async fn refence_pending_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFileRecipientGrantRead,
    ) -> Result<()> {
        self.check_account(pending.recipient)?;
        if !pending.share {
            return Err(FileRecipientGrantStoreError::InvalidInput);
        }
        pending.schema.verify(self, tx, false).await?;
        // Fixed writer order, before all row locks: existing revision locks cannot fence a
        // missing-revision append while final source authority checks are waiting.
        sqlx::query("LOCK TABLE collaboration_file_recipient_grant_schema, collaboration_file_recipient_grants,
            collaboration_file_recipient_grant_changes IN SHARE MODE").execute(&mut **tx).await?;
        pending.schema.verify(self, tx, false).await?;
        sqlx::query(
            "SELECT singleton FROM collaboration_file_recipient_grant_schema LIMIT 2 FOR SHARE",
        )
        .execute(&mut **tx)
        .await?;
        self.verify_pending_in_transaction(tx, pending).await
    }

    pub(crate) async fn get_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        grant_id: FileGrantId,
        recipient: FileAccountRef,
    ) -> Result<PendingFileRecipientGrantRead> {
        self.get_pending(tx, grant_id, recipient, true).await
    }

    pub(super) async fn get_pending(
        &self,
        tx: &mut Tx<'_>,
        grant_id: FileGrantId,
        recipient: FileAccountRef,
        share: bool,
    ) -> Result<PendingFileRecipientGrantRead> {
        self.check_account(recipient)?;
        let schema = self.check_schema(tx, false).await?;
        if share {
            // Fence ordinary DML in writer order before any row locks. Existing history rows
            // alone cannot block appending a missing revision during final Session SQL waits.
            sqlx::query("LOCK TABLE collaboration_file_recipient_grant_schema, collaboration_file_recipient_grants,
                collaboration_file_recipient_grant_changes IN SHARE MODE").execute(&mut **tx).await?;
            schema.verify(self, tx, false).await?;
            sqlx::query(
                "SELECT singleton FROM collaboration_file_recipient_grant_schema LIMIT 2 FOR SHARE",
            )
            .execute(&mut **tx)
            .await?;
            schema.verify(self, tx, false).await?;
        }
        let selected = self
            .read_selected(tx, grant_id, Some(recipient), share)
            .await?;
        let expected = selected
            .map(|selected| -> Result<_> {
                let digest = selected.fingerprint()?;
                Ok((selected, digest))
            })
            .transpose()?;
        schema.verify(self, tx, false).await?;
        Ok(PendingFileRecipientGrantRead {
            schema,
            grant_id,
            recipient,
            expected,
            share,
        })
    }

    pub(crate) async fn verify_pending_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFileRecipientGrantRead,
    ) -> Result<()> {
        self.check_account(pending.recipient)?;
        pending.schema.verify(self, tx, false).await?;
        let current = self
            .read_selected(tx, pending.grant_id, Some(pending.recipient), pending.share)
            .await?
            .map(|selected| -> Result<_> {
                let digest = selected.fingerprint()?;
                Ok((selected, digest))
            })
            .transpose()?;
        if current != pending.expected {
            return Err(FileRecipientGrantStoreError::InvalidRecord);
        }
        pending.schema.verify(self, tx, false).await?;
        Ok(())
    }
}
