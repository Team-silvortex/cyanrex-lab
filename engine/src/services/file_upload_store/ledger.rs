use super::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Ledger {
    entries: Vec<(FileUploadId, [u8; 32])>,
    versions: HashSet<(FileId, FileVersionId)>,
    owners: HashMap<PrincipalRef, (u64, u32)>,
    total_bytes: u64,
    owner_bytes: u64,
    owner_uploads: u32,
}

impl Ledger {
    pub(super) fn contains(
        &self,
        upload_id: FileUploadId,
        manifest: &FileTransferManifest,
    ) -> bool {
        self.entries.iter().any(|entry| entry.0 == upload_id)
            || self
                .versions
                .contains(&(manifest.file_id(), manifest.version_id()))
    }

    pub(super) fn check_quota(&self, limits: FileUploadLimits, byte_length: u64) -> Result<()> {
        if self
            .total_bytes
            .checked_add(byte_length)
            .is_none_or(|value| value > limits.total_bytes)
            || self
                .owner_bytes
                .checked_add(byte_length)
                .is_none_or(|value| value > limits.owner_bytes)
            || self.entries.len() >= limits.total_uploads as usize
            || self.owner_uploads >= limits.owner_uploads
        {
            return Err(FileUploadStoreError::QuotaExceeded);
        }
        Ok(())
    }

    pub(super) fn add(
        &mut self,
        record: &FileUploadReservation,
        fingerprint: [u8; 32],
        owner: PrincipalRef,
    ) -> Result<()> {
        if !self
            .versions
            .insert((record.manifest.file_id(), record.manifest.version_id()))
            || self.entries.iter().any(|entry| entry.0 == record.upload_id)
        {
            return Err(FileUploadStoreError::InvalidRecord);
        }
        self.total_bytes = self
            .total_bytes
            .checked_add(record.manifest.byte_length())
            .ok_or(FileUploadStoreError::InvalidRecord)?;
        let usage = self.owners.entry(record.owner).or_insert((0, 0));
        usage.0 = usage
            .0
            .checked_add(record.manifest.byte_length())
            .ok_or(FileUploadStoreError::InvalidRecord)?;
        usage.1 = usage
            .1
            .checked_add(1)
            .ok_or(FileUploadStoreError::InvalidRecord)?;
        if record.owner == owner {
            self.owner_bytes = self
                .owner_bytes
                .checked_add(record.manifest.byte_length())
                .ok_or(FileUploadStoreError::InvalidRecord)?;
            self.owner_uploads = self
                .owner_uploads
                .checked_add(1)
                .ok_or(FileUploadStoreError::InvalidRecord)?;
        }
        self.entries.push((record.upload_id, fingerprint));
        self.entries.sort_by_key(|entry| entry.0);
        Ok(())
    }
}

impl FileUploadStore {
    pub(super) async fn ledger(&self, tx: &mut Tx<'_>, owner: PrincipalRef) -> Result<Ledger> {
        // Copy at most 257 scalar UUIDs, then only one bounded manifest body at a time.
        let ids = sqlx::query(
            "SELECT upload_id FROM collaboration_file_uploads ORDER BY upload_id LIMIT 257",
        )
        .fetch_all(&mut **tx)
        .await?;
        if ids.len() > MAX_FILE_UPLOAD_INTENTS as usize {
            return Err(FileUploadStoreError::InvalidRecord);
        }
        let mut ledger = Ledger {
            entries: Vec::with_capacity(ids.len()),
            versions: HashSet::new(),
            owners: HashMap::new(),
            total_bytes: 0,
            owner_bytes: 0,
            owner_uploads: 0,
        };
        let mut previous = None;
        for row in ids {
            let id = FileUploadId::try_from(row.try_get::<Uuid, _>("upload_id")?)
                .map_err(|_| FileUploadStoreError::InvalidRecord)?;
            if previous.is_some_and(|previous| id <= previous) {
                return Err(FileUploadStoreError::InvalidRecord);
            }
            previous = Some(id);
            let (record, fingerprint) = self
                .read_record(tx, id, None)
                .await?
                .ok_or(FileUploadStoreError::InvalidRecord)?;
            ledger.add(&record, fingerprint, owner)?;
        }
        if ledger.total_bytes > self.limits.total_bytes
            || ledger.entries.len() > self.limits.total_uploads as usize
            || ledger.owner_bytes > self.limits.owner_bytes
            || ledger.owner_uploads > self.limits.owner_uploads
            || ledger.owners.values().any(|usage| {
                usage.0 > self.limits.owner_bytes || usage.1 > self.limits.owner_uploads
            })
        {
            return Err(FileUploadStoreError::InvalidRecord);
        }
        Ok(ledger)
    }
}
