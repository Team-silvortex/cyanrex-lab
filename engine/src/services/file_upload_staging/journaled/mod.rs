//! Explicit journaled local staging, never authentication, restoration or permission to retry.
//!
//! An acknowledged Unknown precedes file dispatch. Every occupied coordinate conflicts forever;
//! neither a changed nonce nor matching current bytes permit adoption or automatic retry. All
//! admitted errors/cancellation leave the caller Unknown, even if final metadata already committed.
//! Recorded Confirmed is historical protocol metadata, not a promise of current file health.
use super::{
    FileUploadStaging, UploadChunkAttempt, UploadChunkError, UploadChunkReceipt, UploadChunkState,
};
use crate::{
    models::collaboration::*,
    services::file_upload_chunk_journal::{
        ChunkCoordinate, FileUploadChunkJournal, FileUploadChunkJournalError, JournalSchemaPin,
    },
};
use std::{io::Read, sync::Arc};

mod confirmation;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JournaledUploadChunkError {
    #[error(transparent)]
    Local(#[from] UploadChunkError),
    #[error(transparent)]
    Journal(#[from] FileUploadChunkJournalError),
    #[error("journaled upload chunk outcome unconfirmed")]
    Unconfirmed,
}

struct Inner {
    local: FileUploadStaging,
    journal: FileUploadChunkJournal,
}

#[derive(Clone)]
pub struct JournaledFileUploadStaging {
    inner: Arc<Inner>,
}

pub struct JournaledUploadChunkAttempt {
    issuer: Arc<Inner>,
    local: UploadChunkAttempt,
    journal_pin: JournalSchemaPin,
    coordinate: ChunkCoordinate,
}

impl JournaledUploadChunkAttempt {
    pub fn upload_id(&self) -> FileUploadId {
        self.local.upload_id()
    }

    pub fn owner(&self) -> PrincipalRef {
        self.local.owner()
    }

    pub fn account_id(&self) -> LegacyAccountId {
        self.local.account_id()
    }

    pub fn manifest(&self) -> &FileTransferManifest {
        self.local.manifest()
    }

    pub fn chunk_index(&self) -> u32 {
        self.local.chunk_index()
    }

    pub fn state(&self) -> UploadChunkState {
        self.local.state()
    }
}

impl JournaledFileUploadStaging {
    /// Freezes trusted handles without I/O. Clones share issuer identity and the local controls.
    pub fn new(
        local: FileUploadStaging,
        journal: FileUploadChunkJournal,
    ) -> Result<Self, JournaledUploadChunkError> {
        if local.inner.store.scope() != journal.scope()
            || local.inner.store.namespace() == journal.namespace()
        {
            return Err(UploadChunkError::InvalidInput.into());
        }
        Ok(Self {
            inner: Arc::new(Inner { local, journal }),
        })
    }

    /// Captures original immutable source metadata and journal identity, not a chunk receipt.
    pub async fn prepare(
        &self,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
        index: u32,
    ) -> Result<JournaledUploadChunkAttempt, JournaledUploadChunkError> {
        let _permit = self.inner.local.admit()?;
        let local = self
            .inner
            .local
            .prepare_admitted(upload_id, owner, account_id, index)
            .await?;
        let journal_pin = self.inner.journal.capture_pin().await?;
        let coordinate = ChunkCoordinate::new(&local.pin, local.key, index)?;
        Ok(JournaledUploadChunkAttempt {
            issuer: Arc::clone(&self.inner),
            local,
            journal_pin,
            coordinate,
        })
    }

    /// Admitted attempts become terminal before any awaited storage or file operation. A journal
    /// acknowledgement is required before file dispatch and again before the final observations.
    /// No SQL transaction spans file work; final failures never downgrade or adopt committed rows.
    pub async fn stage_chunk<R: Read + Send + 'static>(
        &self,
        attempt: &mut JournaledUploadChunkAttempt,
        reader: R,
    ) -> Result<UploadChunkReceipt, JournaledUploadChunkError> {
        if !Arc::ptr_eq(&self.inner, &attempt.issuer) || attempt.state() != UploadChunkState::Ready
        {
            return Err(UploadChunkError::InvalidInput.into());
        }
        let _permit = self.inner.local.admit()?;
        attempt.local.state = UploadChunkState::Unknown;
        self.verify_source(attempt).await?;
        let unknown = self
            .inner
            .journal
            .reserve_unknown(&attempt.journal_pin, &attempt.coordinate)
            .await?;
        // The journal commit may have waited. Never dispatch files from an earlier source snapshot.
        self.verify_source(attempt).await?;
        let proof = self
            .inner
            .local
            .inner
            .staging
            .stage_bound_chunk(
                attempt.local.key,
                attempt.manifest(),
                attempt.chunk_index(),
                reader,
            )
            .await
            .map_err(confirmation::initial_stage_error)?;
        let proof = self.verify_local_proof(attempt, proof).await?;
        self.inner
            .journal
            .confirm(&unknown)
            .await
            .map_err(|_| JournaledUploadChunkError::Unconfirmed)?;
        // The original object stays retained across journal waits and COMMIT acknowledgement.
        // Later failures may coexist with a recorded Confirmed row, never with retry permission.
        let proof = self.verify_local_proof(attempt, proof).await?;
        let receipt = UploadChunkReceipt {
            reservation: attempt.local.pin.reservation().clone(),
            index: attempt.chunk_index(),
            _proof: proof,
        };
        // No awaited operation follows the final observations before preserving the real receipt.
        attempt.local.state = UploadChunkState::Confirmed;
        Ok(receipt)
    }
}
