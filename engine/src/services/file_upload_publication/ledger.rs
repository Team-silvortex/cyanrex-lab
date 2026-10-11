//! Complete publication metadata with only scalar fingerprints retained between body reads.
use super::*;
use records::PublishedRow;
use std::collections::HashSet;

#[derive(PartialEq, Eq)]
pub(super) struct Ledger {
    entries: Vec<(FileUploadId, [u8; 32])>,
    versions: HashSet<(FileId, FileVersionId)>,
    nonces: HashSet<Uuid>,
}

impl Ledger {
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn contains(&self, coordinate: &PublicationCoordinate) -> bool {
        let record = &coordinate.record;
        self.entries
            .binary_search_by_key(&record.upload_id, |entry| entry.0)
            .is_ok()
            || self
                .versions
                .contains(&(record.manifest.file_id(), record.manifest.version_id()))
    }

    pub(super) fn add(&mut self, row: &PublishedRow, fingerprint: [u8; 32]) -> Result<()> {
        let record = &row.coordinate.record;
        let Err(position) = self
            .entries
            .binary_search_by_key(&record.upload_id, |entry| entry.0)
        else {
            return Err(FileUploadPublicationStoreError::InvalidRecord);
        };
        if !self
            .versions
            .insert((record.manifest.file_id(), record.manifest.version_id()))
            || !self.nonces.insert(row.nonce)
        {
            return Err(FileUploadPublicationStoreError::InvalidRecord);
        }
        self.entries
            .insert(position, (record.upload_id, fingerprint));
        Ok(())
    }
}

impl FileUploadPublicationStore {
    pub(super) async fn ledger(&self, tx: &mut Tx<'_>) -> Result<Ledger> {
        // At most max+1 scalar heads, then only one bounded complete manifest at a time.
        let heads = sqlx::query(
            "SELECT upload_id FROM collaboration_file_publications ORDER BY upload_id LIMIT $1",
        )
        .bind(i64::from(self.max_publications) + 1)
        .fetch_all(&mut **tx)
        .await?;
        if heads.len() > self.max_publications as usize {
            return Err(FileUploadPublicationStoreError::InvalidRecord);
        }
        let mut ledger = Ledger {
            entries: Vec::with_capacity(heads.len()),
            versions: HashSet::new(),
            nonces: HashSet::new(),
        };
        let mut previous = None;
        for head in heads {
            let id = FileUploadId::try_from(head.try_get::<Uuid, _>("upload_id")?)
                .map_err(|_| FileUploadPublicationStoreError::InvalidRecord)?;
            if previous.is_some_and(|previous| id <= previous) {
                return Err(FileUploadPublicationStoreError::InvalidRecord);
            }
            previous = Some(id);
            let (row, digest) = self
                .read_record(tx, id, None)
                .await?
                .ok_or(FileUploadPublicationStoreError::InvalidRecord)?;
            ledger.add(&row, digest)?;
        }
        Ok(ledger)
    }
}
