//! Complete bounded compact metadata, not a proof of every historical file or source manifest.
use super::*;
use records::{ChunkRow, PROJECTION};
use std::collections::{HashMap, HashSet};

#[derive(Clone, PartialEq, Eq)]
struct UploadBinding {
    owner: PrincipalRef,
    account: LegacyAccountId,
    source: [u8; 32],
    root: [u8; 32],
    total_length: u64,
}

impl UploadBinding {
    fn of(coordinate: &ChunkCoordinate) -> Self {
        Self {
            owner: coordinate.record.owner,
            account: coordinate.record.account_id,
            source: coordinate.source_key,
            root: coordinate.root_key,
            total_length: coordinate.total_length,
        }
    }
}

#[derive(PartialEq, Eq)]
pub(super) struct Ledger {
    entries: Vec<(FileUploadId, u32, [u8; 32])>,
    bindings: HashMap<FileUploadId, UploadBinding>,
    nonces: HashSet<Uuid>,
}

impl Ledger {
    pub(super) fn contains(&self, coordinate: &ChunkCoordinate) -> bool {
        self.entries
            .binary_search_by_key(
                &(coordinate.record.upload_id, coordinate.record.index),
                |entry| (entry.0, entry.1),
            )
            .is_ok()
    }

    pub(super) fn compatible(&self, coordinate: &ChunkCoordinate) -> bool {
        self.bindings
            .get(&coordinate.record.upload_id)
            .is_none_or(|binding| *binding == UploadBinding::of(coordinate))
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn add(&mut self, row: &ChunkRow) -> Result<()> {
        let coordinate = &row.coordinate;
        if self.contains(coordinate)
            || !self.compatible(coordinate)
            || !self.nonces.insert(row.nonce)
        {
            return Err(FileUploadChunkJournalError::InvalidRecord);
        }
        self.bindings
            .entry(coordinate.record.upload_id)
            .or_insert_with(|| UploadBinding::of(coordinate));
        let position = self
            .entries
            .binary_search_by_key(
                &(coordinate.record.upload_id, coordinate.record.index),
                |entry| (entry.0, entry.1),
            )
            .map_or_else(|position| position, |position| position);
        self.entries.insert(
            position,
            (
                coordinate.record.upload_id,
                coordinate.record.index,
                row.fingerprint(),
            ),
        );
        Ok(())
    }

    pub(super) fn confirm(&mut self, before: &ChunkRow, after: &ChunkRow) -> Result<()> {
        let record = &before.coordinate.record;
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.0 == record.upload_id && entry.1 == record.index)
            .ok_or(FileUploadChunkJournalError::InvalidRecord)?;
        if entry.2 != before.fingerprint() {
            return Err(FileUploadChunkJournalError::InvalidRecord);
        }
        entry.2 = after.fingerprint();
        Ok(())
    }
}

impl FileUploadChunkJournal {
    pub(super) async fn ledger(&self, tx: &mut Tx<'_>) -> Result<Ledger> {
        let query = format!(
            "SELECT {PROJECTION} FROM collaboration_file_upload_chunks
            ORDER BY upload_id, chunk_index LIMIT $1"
        );
        let rows = sqlx::query(&query)
            .bind(i64::from(self.max_entries) + 1)
            .fetch_all(&mut **tx)
            .await?;
        if rows.len() > self.max_entries as usize {
            return Err(FileUploadChunkJournalError::InvalidRecord);
        }
        let mut ledger = Ledger {
            entries: Vec::with_capacity(rows.len()),
            bindings: HashMap::new(),
            nonces: HashSet::new(),
        };
        let mut previous = None;
        for raw in rows {
            let row = ChunkRow::decode(&raw, self.scope)?;
            let key = (row.coordinate.record.upload_id, row.coordinate.record.index);
            if previous.is_some_and(|previous| key <= previous) {
                return Err(FileUploadChunkJournalError::InvalidRecord);
            }
            previous = Some(key);
            ledger.add(&row)?;
        }
        Ok(ledger)
    }
}
