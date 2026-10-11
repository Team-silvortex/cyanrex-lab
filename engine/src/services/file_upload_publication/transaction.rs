//! Borrowed metadata reads never manufacture an original publication acknowledgement.
use super::*;
use records::PublishedRow;

/// Move-only selected-row observation, including its exact hidden keys and original nonce.
/// Consume only after the caller rechecks this pin, finishes authority checks and confirms COMMIT.
pub(crate) struct PendingFileUploadPublicationRead {
    schema: PublicationSchemaPin,
    upload_id: FileUploadId,
    owner: PrincipalRef,
    account_id: LegacyAccountId,
    expected: Option<(PublishedRow, [u8; 32])>,
    share: bool,
}

impl PendingFileUploadPublicationRead {
    pub(crate) fn into_record(self) -> Option<FileUploadPublicationRecord> {
        self.expected.map(|value| value.0.coordinate.record)
    }
}

impl FileUploadPublicationStore {
    /// No pool acquisition, transaction settings, COMMIT or publication write. A present row is
    /// shared-locked through the caller's final authority wait. None remains a selected observation,
    /// not a promise that another transaction cannot subsequently add a matching record.
    pub(crate) async fn get_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
    ) -> Result<PendingFileUploadPublicationRead> {
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
    ) -> Result<PendingFileUploadPublicationRead> {
        if owner.authority_id != self.scope.authority_id {
            return Err(FileUploadPublicationStoreError::ScopeMismatch);
        }
        let schema = self.check_schema(tx, false).await?;
        if share {
            // Hold the exact original marker through the caller's final authority waits, then
            // fresh-check after acquiring it rather than adopting a changed installation.
            sqlx::query(
                "SELECT singleton FROM collaboration_file_publication_schema LIMIT 2 FOR SHARE",
            )
            .execute(&mut **tx)
            .await?;
            schema.verify(self, tx, false).await?;
        }
        let expected = self
            .read_record_with_lock(tx, upload_id, Some((owner, account_id)), share)
            .await?;
        schema.verify(self, tx, false).await?;
        Ok(PendingFileUploadPublicationRead {
            schema,
            upload_id,
            owner,
            account_id,
            expected,
            share,
        })
    }

    /// Recheck the original namespace/schema before reading the exact selected row. This compares
    /// the full private row and fingerprint, not only the public metadata, and never recaptures an
    /// acknowledgement or claims that every foreign row in the publication ledger is healthy.
    pub(crate) async fn verify_pending_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFileUploadPublicationRead,
    ) -> Result<()> {
        if pending.owner.authority_id != self.scope.authority_id {
            return Err(FileUploadPublicationStoreError::ScopeMismatch);
        }
        pending.schema.verify(self, tx, false).await?;
        let current = self
            .read_record_with_lock(
                tx,
                pending.upload_id,
                Some((pending.owner, pending.account_id)),
                pending.share,
            )
            .await?;
        if current != pending.expected {
            return Err(FileUploadPublicationStoreError::InvalidRecord);
        }
        pending.schema.verify(self, tx, false).await?;
        Ok(())
    }
}
