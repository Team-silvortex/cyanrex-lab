//! Borrowed publication mutation. The source caller owns settings, flush and COMMIT.
use super::*;
use ledger::Ledger;
use records::{canonical_manifest, fingerprint, PublishedRow};

/// A pending insert is not a publication acknowledgement. Consume only after COMMIT ACK.
pub(crate) struct PendingFileUploadPublicationMutation {
    schema: PublicationSchemaPin,
    expected: PublishedRow,
    fingerprint: [u8; 32],
    ledger: Ledger,
    selected_namespace: bool,
}

impl PendingFileUploadPublicationMutation {
    pub(crate) fn into_acknowledgement(self) -> PublicationAcknowledgement {
        PublicationAcknowledgement {
            schema: self.schema,
            expected: self.expected,
            fingerprint: self.fingerprint,
        }
    }

    pub(crate) fn is_for(&self, coordinate: &PublicationCoordinate) -> bool {
        self.expected.coordinate == *coordinate
    }
}

impl FileUploadPublicationStore {
    pub(crate) async fn publish_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pin: &PublicationSchemaPin,
        coordinate: &PublicationCoordinate,
    ) -> Result<PendingFileUploadPublicationMutation> {
        self.publish_pending_in_transaction(tx, pin, coordinate, true)
            .await
    }

    pub(super) async fn publish_pending_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pin: &PublicationSchemaPin,
        coordinate: &PublicationCoordinate,
        selected_namespace: bool,
    ) -> Result<PendingFileUploadPublicationMutation> {
        let record = &coordinate.record;
        if record.owner.authority_id != self.scope.authority_id
            || record.manifest.workspace() != self.scope
        {
            return Err(FileUploadPublicationStoreError::ScopeMismatch);
        }
        let raw = canonical_manifest(&record.manifest)?;
        let row = PublishedRow {
            coordinate: coordinate.clone(),
            nonce: Uuid::new_v4(),
        };
        let record_digest = fingerprint(&row, &raw);
        self.verify_mutation_schema(tx, pin, selected_namespace)
            .await?;
        let mut expected = self.ledger(tx).await?;
        // Validate every retained row first; every valid upload/version duplicate conflicts
        // before capacity regardless of its owner, bytes or private source/root/journal binding.
        if expected.contains(coordinate) {
            return Err(FileUploadPublicationStoreError::Conflict);
        }
        if expected.len() >= self.max_publications as usize {
            return Err(FileUploadPublicationStoreError::CapacityExhausted);
        }
        expected.add(&row, record_digest)?;
        let written = sqlx::query("INSERT INTO collaboration_file_publications
            (upload_id,file_id,version_id,owner_id,account_id,manifest,source_key,root_key,journal_key,nonce)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(record.upload_id.as_uuid()).bind(record.manifest.file_id().as_uuid()).bind(record.manifest.version_id().as_uuid())
            .bind(record.owner.principal_id.as_uuid()).bind(record.account_id.as_uuid()).bind(&raw)
            .bind(coordinate.source_key.as_slice()).bind(coordinate.root_key.as_slice()).bind(coordinate.journal_key.as_slice())
            .bind(row.nonce).execute(&mut **tx).await?.rows_affected();
        if written != 1 {
            return Err(FileUploadPublicationStoreError::InvalidRecord);
        }
        Ok(PendingFileUploadPublicationMutation {
            schema: pin.retain(),
            expected: row,
            fingerprint: record_digest,
            ledger: expected,
            selected_namespace,
        })
    }

    pub(crate) async fn verify_mutation_in_transaction(
        &self,
        tx: &mut Tx<'_>,
        pending: &PendingFileUploadPublicationMutation,
    ) -> Result<()> {
        let record = &pending.expected.coordinate.record;
        if record.owner.authority_id != self.scope.authority_id
            || record.manifest.workspace() != self.scope
        {
            return Err(FileUploadPublicationStoreError::ScopeMismatch);
        }
        // The caller flushes deferred work first. Never reset search_path or adopt a new pin.
        self.verify_mutation_schema(tx, &pending.schema, pending.selected_namespace)
            .await?;
        let (stored, digest) = self
            .read_record(tx, record.upload_id, None)
            .await?
            .ok_or(FileUploadPublicationStoreError::InvalidRecord)?;
        if stored != pending.expected
            || digest != pending.fingerprint
            || self.ledger(tx).await? != pending.ledger
        {
            return Err(FileUploadPublicationStoreError::InvalidRecord);
        }
        self.verify_mutation_schema(tx, &pending.schema, pending.selected_namespace)
            .await?;
        Ok(())
    }

    async fn verify_mutation_schema(
        &self,
        tx: &mut Tx<'_>,
        pin: &PublicationSchemaPin,
        selected: bool,
    ) -> Result<()> {
        if selected {
            pin.verify_selected(self, tx, true).await
        } else {
            pin.verify(self, tx, true).await
        }
    }
}
