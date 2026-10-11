use super::*;

impl FileUploadStore {
    /// Reserve one immutable intent. Owners/account IDs are trusted attribution, not credentials.
    /// A repeat upload ID or immutable file/version always conflicts, even for identical metadata.
    pub async fn reserve(
        &self,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
        manifest: &FileTransferManifest,
    ) -> Result<FileUploadReservation> {
        self.check_owner(owner)?;
        if manifest.workspace() != self.scope {
            return Err(FileUploadStoreError::ScopeMismatch);
        }
        bounded(async {
            let mut tx = self.begin(false).await?;
            let pending = self
                .reserve_in_transaction(&mut tx, upload_id, owner, account_id, manifest)
                .await?;
            // Run pending deferred constraint work before readback, then revalidate the ledger.
            // Database administrators and arbitrary trigger code remain trusted.
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
                .execute(&mut *tx)
                .await?;
            self.verify_pending_in_transaction(&mut tx, &pending)
                .await?;
            // Triggers may change a local setting without changing any record or schema pin.
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            pending
                .into_record()
                .ok_or(FileUploadStoreError::InvalidRecord)
        })
        .await
    }

    /// Exact owner and account incarnation only. Reading is not authorization or a write receipt.
    /// This validates the selected record, not the health or progress of every retained intent.
    pub async fn get(
        &self,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
    ) -> Result<Option<FileUploadReservation>> {
        self.check_owner(owner)?;
        bounded(async {
            let mut tx = self.begin(true).await?;
            // This original trusted API remains RR READ ONLY with no selected-row lock.
            let pending = self
                .get_pending_in_transaction(&mut tx, upload_id, owner, account_id, false)
                .await?;
            self.verify_pending_in_transaction(&mut tx, &pending)
                .await?;
            tx.commit().await?;
            Ok(pending.into_record())
        })
        .await
    }
}
