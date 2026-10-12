//! Borrowed own-subject metadata; no independent pool, authority check, settings or COMMIT.
use super::*;
use records::Selected;
use schema::SchemaPin;

/// Move-only original schema/head/history snapshot. Data escapes only after the caller verifies
/// it in the selected target namespace, finishes final authority checks and confirms COMMIT.
pub(crate) struct PendingFilePublisherGrantRead {
    schema: SchemaPin,
    grant_id: FileGrantId,
    subject: FileAccountRef,
    expected: Option<(Selected, [u8; 32])>,
    share: bool,
}

impl PendingFilePublisherGrantRead {
    pub(crate) fn into_record(self) -> Option<StoredFilePublisherGrant> {
        self.expected.map(|value| value.0.record)
    }

    /// A deadline bound, not detached authority. The caller must fence and verify this original
    /// selected history in its final transaction and recheck current database time before commit.
    #[cfg(unix)]
    pub(crate) fn active_validity(&self) -> Option<FileGrantValidity> {
        self.expected.as_ref().and_then(|(selected, _)| {
            (selected.record.grant.state() == FileGrantState::Active)
                .then(|| selected.record.grant.validity())
        })
    }
}

impl FilePublisherGrantStore {
    /// Reacquire the original selected grant's write fence in a new caller-owned transaction.
    /// This never captures/adopts a replacement installation or history after a file-work gap.
    #[cfg(unix)]
    pub(crate) async fn refence_pending_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFilePublisherGrantRead,
    ) -> Result<()> {
        pending.schema.verify(self, tx, false).await?;
        sqlx::query("LOCK TABLE collaboration_file_publisher_grant_schema, collaboration_file_publisher_grants,
            collaboration_file_publisher_grant_changes IN SHARE MODE").execute(&mut **tx).await?;
        pending.schema.verify(self, tx, false).await?;
        sqlx::query(
            "SELECT singleton FROM collaboration_file_publisher_grant_schema LIMIT 2 FOR SHARE",
        )
        .execute(&mut **tx)
        .await?;
        self.verify_pending_in_transaction(tx, pending).await
    }

    /// Current selected metadata plus fresh database time, useful only inside this caller's fence.
    #[cfg(unix)]
    pub(crate) async fn permits_pending_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFilePublisherGrantRead,
    ) -> Result<bool> {
        self.verify_pending_in_transaction(tx, pending).await?;
        let now = self.database_now(tx).await?;
        Ok(pending.expected.as_ref().is_some_and(|(selected, _)| {
            selected
                .record
                .grant
                .matches(self.scope, pending.subject, now)
        }))
    }

    pub(crate) async fn get_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        grant_id: FileGrantId,
        subject: FileAccountRef,
    ) -> Result<PendingFilePublisherGrantRead> {
        self.get_pending(tx, grant_id, subject, true).await
    }

    pub(super) async fn get_pending(
        &self,
        tx: &mut Tx<'_>,
        grant_id: FileGrantId,
        subject: FileAccountRef,
        share: bool,
    ) -> Result<PendingFilePublisherGrantRead> {
        self.check_account(subject)?;
        let schema = self.check_schema(tx, false).await?;
        if share {
            // Before taking any row locks, fence ordinary DML on all three tables in the same
            // order as writers. Existing history row locks alone do not protect missing revisions
            // from a phantom append while the caller waits for final Session checks.
            sqlx::query("LOCK TABLE collaboration_file_publisher_grant_schema, collaboration_file_publisher_grants,
                collaboration_file_publisher_grant_changes IN SHARE MODE").execute(&mut **tx).await?;
            schema.verify(self, tx, false).await?;
            sqlx::query(
                "SELECT singleton FROM collaboration_file_publisher_grant_schema LIMIT 2 FOR SHARE",
            )
            .execute(&mut **tx)
            .await?;
            schema.verify(self, tx, false).await?;
        }
        let selected = self
            .read_selected(tx, grant_id, Some(subject), share)
            .await?;
        let expected = selected
            .map(|selected| -> Result<_> {
                let digest = selected.fingerprint()?;
                Ok((selected, digest))
            })
            .transpose()?;
        schema.verify(self, tx, false).await?;
        Ok(PendingFilePublisherGrantRead {
            schema,
            grant_id,
            subject,
            expected,
            share,
        })
    }

    pub(crate) async fn verify_pending_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFilePublisherGrantRead,
    ) -> Result<()> {
        self.check_account(pending.subject)?;
        pending.schema.verify(self, tx, false).await?;
        let current = self
            .read_selected(tx, pending.grant_id, Some(pending.subject), pending.share)
            .await?
            .map(|selected| -> Result<_> {
                let digest = selected.fingerprint()?;
                Ok((selected, digest))
            })
            .transpose()?;
        if current != pending.expected {
            return Err(FilePublisherGrantStoreError::InvalidRecord);
        }
        pending.schema.verify(self, tx, false).await?;
        Ok(())
    }
}
