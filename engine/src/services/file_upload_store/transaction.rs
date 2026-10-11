//! Borrow one caller-owned transaction; pending metadata is not a commit acknowledgement.
use super::*;
use ledger::Ledger;
use records::{canonical_manifest, fingerprint};
use schema::SchemaPin;

/// Move-only exact observation. A caller must verify this while the original namespace is
/// selected, finish its own authority checks, and confirm COMMIT before consuming the metadata.
pub(crate) struct PendingFileUpload {
    schema: SchemaPin,
    upload_id: FileUploadId,
    owner: PrincipalRef,
    account_id: LegacyAccountId,
    expected: Option<(FileUploadReservation, [u8; 32])>,
    ledger: Option<Ledger>,
    share: bool,
}

impl PendingFileUpload {
    pub(crate) fn into_record(self) -> Option<FileUploadReservation> {
        self.expected.map(|value| value.0)
    }
}

impl FileUploadStore {
    /// No pool acquisition, transaction settings, deferred flush or COMMIT. The caller owns all
    /// of those boundaries. The write fence remains held until that caller's transaction ends.
    pub(crate) async fn reserve_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
        manifest: &FileTransferManifest,
    ) -> Result<PendingFileUpload> {
        self.check_owner(owner)?;
        if manifest.workspace() != self.scope {
            return Err(FileUploadStoreError::ScopeMismatch);
        }
        let raw = canonical_manifest(manifest)?;
        let record = FileUploadReservation {
            upload_id,
            owner,
            account_id,
            manifest: manifest.clone(),
        };
        let schema = self.check_schema(tx, true).await?;
        let mut expected_ledger = self.ledger(tx, owner).await?;
        // Validate every retained row before deciding conflict or quota, never omit corrupt usage.
        if expected_ledger.contains(upload_id, manifest) {
            return Err(FileUploadStoreError::Conflict);
        }
        expected_ledger.check_quota(self.limits, manifest.byte_length())?;
        let digest = fingerprint(&record, &raw);
        expected_ledger.add(&record, digest, owner)?;
        confirmed_one(
            sqlx::query(
                "INSERT INTO collaboration_file_uploads
                (upload_id, owner_id, account_id, manifest) VALUES($1, $2, $3, $4)",
            )
            .bind(upload_id.as_uuid())
            .bind(owner.principal_id.as_uuid())
            .bind(account_id.as_uuid())
            .bind(&raw)
            .execute(&mut **tx)
            .await?
            .rows_affected(),
        )?;
        Ok(PendingFileUpload {
            schema,
            upload_id,
            owner,
            account_id,
            expected: Some((record, digest)),
            ledger: Some(expected_ledger),
            share: false,
        })
    }

    /// Selected owner/account observation only, not a complete-ledger audit. A present row is
    /// shared-locked through the caller's final authority wait and COMMIT; absence is a snapshot,
    /// not a promise that another transaction cannot add a record afterwards.
    #[cfg(unix)]
    pub(crate) async fn get_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
    ) -> Result<PendingFileUpload> {
        self.get_pending_in_transaction(tx, upload_id, owner, account_id, true)
            .await
    }

    pub(super) async fn get_pending_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
        share: bool,
    ) -> Result<PendingFileUpload> {
        self.check_owner(owner)?;
        let schema = self.check_schema(tx, false).await?;
        if share {
            // Keep the original settings row stable across the caller's final authority waits.
            // Read fresh after this lock wait before accepting any selected record.
            sqlx::query("SELECT singleton FROM collaboration_file_upload_schema LIMIT 2 FOR SHARE")
                .execute(&mut **tx)
                .await?;
            schema.verify(self, tx, false).await?;
        }
        let expected = self
            .read_record_with_lock(tx, upload_id, Some((owner, account_id)), share)
            .await?;
        schema.verify(self, tx, false).await?;
        Ok(PendingFileUpload {
            schema,
            upload_id,
            owner,
            account_id,
            expected,
            ledger: None,
            share,
        })
    }

    /// Recheck the original namespace before unqualified reads. For a reservation this must run
    /// after the caller flushes deferred constraint work, and compares the entire expected ledger.
    pub(crate) async fn verify_pending_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFileUpload,
    ) -> Result<()> {
        self.check_owner(pending.owner)?;
        let writer = pending.ledger.is_some();
        pending.schema.verify(self, tx, writer).await?;
        let current = self
            .read_record_with_lock(
                tx,
                pending.upload_id,
                Some((pending.owner, pending.account_id)),
                pending.share,
            )
            .await?;
        if current != pending.expected {
            return Err(FileUploadStoreError::InvalidRecord);
        }
        if let Some(expected) = &pending.ledger {
            if self.ledger(tx, pending.owner).await? != *expected {
                return Err(FileUploadStoreError::InvalidRecord);
            }
        }
        pending.schema.verify(self, tx, writer).await?;
        Ok(())
    }
}
