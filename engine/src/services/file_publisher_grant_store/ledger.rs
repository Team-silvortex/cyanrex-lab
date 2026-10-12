//! Complete bounded head/history inventory; retain fingerprints, not a batch of grant bodies.
use super::*;
use records::{revision, Selected};

#[derive(PartialEq, Eq)]
pub(super) struct Ledger {
    entries: Vec<(FileGrantId, [u8; 32])>,
    changes: Vec<(FileGrantId, u64)>,
}

impl Ledger {
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn contains(&self, id: FileGrantId) -> bool {
        self.entries
            .binary_search_by_key(&id, |entry| entry.0)
            .is_ok()
    }

    pub(super) fn add(&mut self, selected: &Selected) -> Result<()> {
        let id = selected.record.grant.id();
        let Err(position) = self.entries.binary_search_by_key(&id, |entry| entry.0) else {
            return Err(FilePublisherGrantStoreError::InvalidRecord);
        };
        self.entries.insert(position, (id, selected.fingerprint()?));
        self.changes.extend(
            selected
                .changes
                .iter()
                .map(|change| (id, u64::from(change.grant.revision()))),
        );
        self.changes.sort_unstable();
        Ok(())
    }

    pub(super) fn replace(&mut self, selected: &Selected) -> Result<()> {
        let id = selected.record.grant.id();
        let position = self
            .entries
            .binary_search_by_key(&id, |entry| entry.0)
            .map_err(|_| FilePublisherGrantStoreError::InvalidRecord)?;
        self.entries[position].1 = selected.fingerprint()?;
        self.changes.retain(|entry| entry.0 != id);
        self.changes.extend(
            selected
                .changes
                .iter()
                .map(|change| (id, u64::from(change.grant.revision()))),
        );
        self.changes.sort_unstable();
        Ok(())
    }
}

impl FilePublisherGrantStore {
    pub(super) async fn ledger(&self, tx: &mut Tx<'_>) -> Result<Ledger> {
        let heads = sqlx::query(
            "SELECT grant_id FROM collaboration_file_publisher_grants ORDER BY grant_id LIMIT $1",
        )
        .bind(i64::from(self.max_grants) + 1)
        .fetch_all(&mut **tx)
        .await?;
        if heads.len() > self.max_grants as usize {
            return Err(FilePublisherGrantStoreError::InvalidRecord);
        }
        let mut ledger = Ledger {
            entries: Vec::with_capacity(heads.len()),
            changes: Vec::new(),
        };
        let mut previous = None;
        for head in heads {
            let id = FileGrantId::try_from(head.try_get::<Uuid, _>("grant_id")?)
                .map_err(|_| FilePublisherGrantStoreError::InvalidRecord)?;
            if previous.is_some_and(|previous| id <= previous) {
                return Err(FilePublisherGrantStoreError::InvalidRecord);
            }
            previous = Some(id);
            let selected = self
                .read_selected(tx, id, None, false)
                .await?
                .ok_or(FilePublisherGrantStoreError::InvalidRecord)?;
            ledger.add(&selected)?;
        }
        // Also inventory every history coordinate, including orphan rows that head selection
        // cannot see. max*2+1 scalar heads are bounded before any historical body is decoded.
        let changes = sqlx::query(
            "SELECT grant_id,revision FROM collaboration_file_publisher_grant_changes
            ORDER BY grant_id,revision LIMIT $1",
        )
        .bind(i64::from(self.max_grants) * 2 + 1)
        .fetch_all(&mut **tx)
        .await?;
        if changes.len() > self.max_grants as usize * 2 {
            return Err(FilePublisherGrantStoreError::InvalidRecord);
        }
        let mut coordinates = Vec::with_capacity(changes.len());
        for change in changes {
            let id = FileGrantId::try_from(change.try_get::<Uuid, _>("grant_id")?)
                .map_err(|_| FilePublisherGrantStoreError::InvalidRecord)?;
            coordinates.push((id, revision(change.try_get("revision")?)?));
        }
        if coordinates != ledger.changes {
            return Err(FilePublisherGrantStoreError::InvalidRecord);
        }
        Ok(ledger)
    }
}
