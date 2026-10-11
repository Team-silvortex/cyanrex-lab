use super::*;
use records::{canonical_manifest, fingerprint, PublishedRow};

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
        let record = &coordinate.record;
        if record.owner.authority_id != self.scope.authority_id
            || record.manifest.workspace() != self.scope
        {
            return Err(FileUploadPublicationStoreError::ScopeMismatch);
        }
        bounded(async {
            let raw = canonical_manifest(&record.manifest)?;
            let row = PublishedRow { coordinate: coordinate.clone(), nonce: Uuid::new_v4() };
            let record_digest = fingerprint(&row, &raw);
            let mut tx = self.begin(false).await?;
            pin.verify(self, &mut tx, true).await?;
            let mut expected = self.ledger(&mut tx).await?;
            // Invalid retained records fail closed first; every valid existing upload or version
            // conflicts before capacity, regardless of matching bytes, owner or source/root keys.
            if expected.contains(coordinate) { return Err(FileUploadPublicationStoreError::Conflict); }
            if expected.len() >= self.max_publications as usize { return Err(FileUploadPublicationStoreError::CapacityExhausted); }
            expected.add(&row, record_digest)?;
            let written = sqlx::query("INSERT INTO collaboration_file_publications
                (upload_id,file_id,version_id,owner_id,account_id,manifest,source_key,root_key,journal_key,nonce)
                VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
                .bind(record.upload_id.as_uuid()).bind(record.manifest.file_id().as_uuid()).bind(record.manifest.version_id().as_uuid())
                .bind(record.owner.principal_id.as_uuid()).bind(record.account_id.as_uuid()).bind(&raw)
                .bind(coordinate.source_key.as_slice()).bind(coordinate.root_key.as_slice()).bind(coordinate.journal_key.as_slice())
                .bind(row.nonce).execute(&mut *tx).await?.rows_affected();
            if written != 1 { return Err(FileUploadPublicationStoreError::InvalidRecord); }
            // A trigger may modify local settings without changing a row. Flush deferred work,
            // then verify the entire exact expected ledger and original namespace before COMMIT.
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            sqlx::query("SET CONSTRAINTS ALL IMMEDIATE").execute(&mut *tx).await?;
            pin.verify(self, &mut tx, true).await?;
            let (stored, digest) = self.read_record(&mut tx, record.upload_id, None).await?
                .ok_or(FileUploadPublicationStoreError::InvalidRecord)?;
            if stored != row || digest != record_digest || self.ledger(&mut tx).await? != expected {
                return Err(FileUploadPublicationStoreError::InvalidRecord);
            }
            pin.verify(self, &mut tx, true).await?;
            sqlx::query("SET LOCAL synchronous_commit = 'on'").execute(&mut *tx).await?;
            tx.commit().await?;
            // No acknowledgement is minted for an unconfirmed COMMIT or restored metadata read.
            Ok(PublicationAcknowledgement { schema: pin.retain(), expected: row, fingerprint: record_digest })
        }).await
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
