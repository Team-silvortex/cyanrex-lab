//! Original, complete chunk confirmation metadata for one prepared publication.
//! This is neither current byte health nor a recovered write acknowledgement.
use super::{
    records::{ChunkRow, PROJECTION},
    *,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

/// Move-only original schema and ordered confirmation fingerprints; no metadata getter can mint it.
pub(crate) struct ConfirmedFilePin {
    schema: JournalSchemaPin,
    upload_id: FileUploadId,
    source_key: [u8; 32],
    root_key: [u8; 32],
    fingerprints: Vec<[u8; 32]>,
    journal_key: [u8; 32],
}

impl ConfirmedFilePin {
    pub(crate) fn journal_key(&self) -> [u8; 32] {
        self.journal_key
    }
}

impl FileUploadChunkJournal {
    /// Reads all bounded target coordinates, without filtering away a mismatched owner or binding.
    /// Missing or Unknown valid rows are not ready; malformed, extra or mixed rows fail closed.
    pub(crate) async fn capture_confirmed_file(
        &self,
        source: &UploadReservationPin,
        root_key: [u8; 32],
    ) -> Result<Option<ConfirmedFilePin>> {
        self.check_publication_source(source)?;
        bounded(async {
            let mut tx = self.begin_publication_read().await?;
            let schema = self.check_schema(&mut tx, false).await?;
            let fingerprints = self
                .confirmed_fingerprints(&mut tx, source, root_key)
                .await?;
            schema.verify(self, &mut tx, false).await?;
            tx.commit().await?;
            Ok(fingerprints.map(|fingerprints| {
                let mut hash = Sha256::new();
                hash.update(b"cyanrex.file-upload.confirmed-file.v1\0");
                hash.update(schema.publication_identity(&self.namespace));
                hash.update(source.reservation().upload_id().as_uuid().as_bytes());
                hash.update(source.source_key());
                hash.update(root_key);
                hash.update((fingerprints.len() as u32).to_be_bytes());
                for fingerprint in &fingerprints {
                    hash.update(fingerprint);
                }
                ConfirmedFilePin {
                    schema,
                    upload_id: source.reservation().upload_id(),
                    source_key: source.source_key(),
                    root_key,
                    fingerprints,
                    journal_key: hash.finalize().into(),
                }
            }))
        })
        .await
    }

    /// Rechecks only the original schema and exact original ordered confirmations, never recaptures.
    pub(crate) async fn verify_confirmed_file(
        &self,
        source: &UploadReservationPin,
        root_key: [u8; 32],
        pin: &ConfirmedFilePin,
    ) -> Result<()> {
        self.check_publication_source(source)?;
        if pin.upload_id != source.reservation().upload_id()
            || pin.source_key != source.source_key()
            || pin.root_key != root_key
        {
            return Err(FileUploadChunkJournalError::InvalidInput);
        }
        bounded(async {
            let mut tx = self.begin_publication_read().await?;
            pin.schema.verify(self, &mut tx, false).await?;
            let current = self
                .confirmed_fingerprints(&mut tx, source, root_key)
                .await?
                .ok_or(FileUploadChunkJournalError::InvalidRecord)?;
            if current != pin.fingerprints {
                return Err(FileUploadChunkJournalError::InvalidRecord);
            }
            pin.schema.verify(self, &mut tx, false).await?;
            tx.commit().await?;
            Ok(())
        })
        .await
    }

    fn check_publication_source(&self, source: &UploadReservationPin) -> Result<()> {
        let reservation = source.reservation();
        if reservation.manifest().workspace() != self.scope
            || reservation.owner().authority_id != self.scope.authority_id
        {
            return Err(FileUploadChunkJournalError::ScopeMismatch);
        }
        Ok(())
    }

    async fn begin_publication_read(&self) -> Result<Tx<'_>> {
        let mut tx = self.begin(true).await?;
        // The original relation check can wait on a writer: do not retain a pre-wait RR snapshot.
        // READ ONLY remains in effect, and this precedes the first SELECT.
        sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }

    async fn confirmed_fingerprints(
        &self,
        tx: &mut Tx<'_>,
        source: &UploadReservationPin,
        root_key: [u8; 32],
    ) -> Result<Option<Vec<[u8; 32]>>> {
        let manifest = source.reservation().manifest();
        let expected_count = manifest.chunk_sha256().len();
        let query = format!(
            "SELECT {PROJECTION} FROM collaboration_file_upload_chunks
            WHERE upload_id=$1 ORDER BY chunk_index LIMIT $2"
        );
        let rows = sqlx::query(&query)
            .bind(source.reservation().upload_id().as_uuid())
            .bind((expected_count + 1) as i64)
            .fetch_all(&mut **tx)
            .await?;
        let mut fingerprints = Vec::with_capacity(rows.len().min(expected_count));
        let mut nonces = HashSet::with_capacity(rows.len());
        let mut previous = None;
        let mut ready = rows.len() == expected_count;
        for row in &rows {
            let row = ChunkRow::decode(row, self.scope)?;
            let index = row.coordinate.record.index;
            if index as usize >= expected_count
                || previous.is_some_and(|previous| index <= previous)
                || !nonces.insert(row.nonce)
            {
                return Err(FileUploadChunkJournalError::InvalidRecord);
            }
            previous = Some(index);
            let mut expected = ChunkCoordinate::new(source, root_key, index)
                .map_err(|_| FileUploadChunkJournalError::InvalidRecord)?;
            expected.record.state = row.coordinate.record.state;
            if row.coordinate != expected {
                return Err(FileUploadChunkJournalError::InvalidRecord);
            }
            ready &= row.coordinate.record.state == PersistentChunkState::Confirmed;
            fingerprints.push(row.fingerprint());
        }
        if rows.len() > expected_count || rows.len() > self.max_entries as usize {
            return Err(FileUploadChunkJournalError::InvalidRecord);
        }
        Ok(ready.then_some(fingerprints))
    }
}
