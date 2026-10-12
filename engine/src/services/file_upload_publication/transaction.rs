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
    exact: Option<(FileId, FileVersionId)>,
    expected: Option<(PublishedRow, [u8; 32])>,
    share: bool,
}

impl PendingFileUploadPublicationRead {
    /// Borrowed selected metadata only. The original hidden row/pins remain retained for final
    /// verification; this view cannot mint an acknowledgement or authorize byte access.
    pub(crate) fn record(&self) -> Option<&FileUploadPublicationRecord> {
        self.expected
            .as_ref()
            .map(|value| &value.0.coordinate.record)
    }

    /// Only the first selected stored source/root coordinates, never a caller path or read grant.
    #[cfg(unix)]
    pub(crate) fn source_root_binding(&self) -> Option<([u8; 32], [u8; 32])> {
        self.expected
            .as_ref()
            .map(|value| (value.0.coordinate.source_key, value.0.coordinate.root_key))
    }

    pub(crate) fn into_record(self) -> Option<FileUploadPublicationRecord> {
        self.expected.map(|value| value.0.coordinate.record)
    }
}

impl FileUploadPublicationStore {
    /// Reacquire the same original marker/selected row after a caller-owned file-work gap.
    /// The nonce, hidden coordinates, exact selector and full fingerprint are never recaptured.
    #[cfg(unix)]
    pub(crate) async fn refence_pending_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFileUploadPublicationRead,
    ) -> Result<()> {
        if !pending.share {
            return Err(FileUploadPublicationStoreError::InvalidInput);
        }
        pending.schema.verify(self, tx, false).await?;
        sqlx::query(
            "SELECT singleton FROM collaboration_file_publication_schema LIMIT 2 FOR SHARE",
        )
        .execute(&mut **tx)
        .await?;
        self.verify_pending_in_transaction(tx, pending).await
    }

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

    /// Exact-version selection only. The caller supplies coordinates derived from its original
    /// selected grant, not a publisher claim; this helper does not authenticate the recipient.
    pub(crate) async fn get_exact_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
        file: &FileVersionRef,
    ) -> Result<PendingFileUploadPublicationRead> {
        if file.workspace() != self.scope {
            return Err(FileUploadPublicationStoreError::ScopeMismatch);
        }
        self.get_pending_exact_in_transaction(
            tx,
            upload_id,
            owner,
            account_id,
            Some((file.file_id(), file.version_id())),
            true,
        )
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
        self.get_pending_exact_in_transaction(tx, upload_id, owner, account_id, None, share)
            .await
    }

    async fn get_pending_exact_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
        exact: Option<(FileId, FileVersionId)>,
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
            .read_record_exact_with_lock(tx, upload_id, Some((owner, account_id)), exact, share)
            .await?;
        schema.verify(self, tx, false).await?;
        Ok(PendingFileUploadPublicationRead {
            schema,
            upload_id,
            owner,
            account_id,
            exact,
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
            .read_record_exact_with_lock(
                tx,
                pending.upload_id,
                Some((pending.owner, pending.account_id)),
                pending.exact,
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
