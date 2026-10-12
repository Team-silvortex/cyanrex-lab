use super::*;

impl FileUploadPublicationStore {
    /// Exact trusted owner/account attribution only; a metadata read is not a commit acknowledgement.
    pub async fn get(
        &self,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
    ) -> Result<Option<FileUploadPublicationRecord>> {
        if owner.authority_id != self.scope.authority_id {
            return Err(FileUploadPublicationStoreError::ScopeMismatch);
        }
        bounded(async {
            let mut tx = self.begin(true).await?;
            // Preserve this original trusted API's RR READ ONLY and unlocked selected read.
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

    /// Exactly one immutable row. The original captured pin is checked in this write transaction;
    /// a separate current capture must not hide identity drift or move the commit boundary.
    pub(crate) async fn publish(
        &self,
        pin: &PublicationSchemaPin,
        coordinate: &PublicationCoordinate,
    ) -> Result<PublicationAcknowledgement> {
        if coordinate.record.owner.authority_id != self.scope.authority_id
            || coordinate.record.manifest.workspace() != self.scope
        {
            return Err(FileUploadPublicationStoreError::ScopeMismatch);
        }
        bounded(async {
            let mut tx = self.begin(false).await?;
            let pending = self
                .publish_pending_in_transaction(&mut tx, pin, coordinate, false)
                .await?;
            // A trigger may modify local settings without changing a row. Flush deferred work,
            // then verify the entire exact expected ledger and original namespace before COMMIT.
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
                .execute(&mut *tx)
                .await?;
            self.verify_mutation_in_transaction(&mut tx, &pending)
                .await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'")
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            // No acknowledgement is minted for an unconfirmed COMMIT or restored metadata read.
            Ok(pending.into_acknowledgement())
        })
        .await
    }

    /// Fresh exact original-row/schema observation only. This neither recaptures a replacement
    /// installation nor turns a matching public lookup into an original acknowledgement.
    pub(crate) async fn verify_ack(&self, ack: &PublicationAcknowledgement) -> Result<()> {
        let expected = &ack.expected;
        if expected.coordinate.record.owner.authority_id != self.scope.authority_id
            || expected.coordinate.record.manifest.workspace() != self.scope
        {
            return Err(FileUploadPublicationStoreError::ScopeMismatch);
        }
        bounded(async {
            let mut tx = self.begin(true).await?;
            // The shared reader starts RR READ ONLY. Keep that policy but read fresh rows after
            // any original-table lock wait rather than accepting a pre-wait namespace snapshot.
            sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
                .execute(&mut *tx)
                .await?;
            ack.schema.verify(self, &mut tx, false).await?;
            let (stored, digest) = self
                .read_record(&mut tx, expected.coordinate.record.upload_id, None)
                .await?
                .ok_or(FileUploadPublicationStoreError::InvalidRecord)?;
            if stored != *expected || digest != ack.fingerprint {
                return Err(FileUploadPublicationStoreError::InvalidRecord);
            }
            ack.schema.verify(self, &mut tx, false).await?;
            tx.commit().await?;
            Ok(())
        })
        .await
    }
}
