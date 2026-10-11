//! Explicit upload-bound local staging, not authentication or a persistent chunk journal.
//!
//! Preparation reads trusted intent metadata only. One caller-owned attempt permits one dispatch;
//! after stage control admission it is Unknown even when pre-write validation fails. A receipt confirms
//! this local fsync/readback and later source/entry observations, not publication, durable progress
//! or full-file integrity. No attempt can be serialized, restored, adopted or automatically retried.
//! Independently prepared attempts still use the same exclusive destination, not a random nonce.
//! Database and filesystem observations are separate, never an atomic cross-storage transaction.
//! The separate journaled wrapper additionally requires write-ahead and final journal acknowledgements;
//! its persisted metadata is neither a restored attempt nor proof that file bytes remain available.
use super::{
    file_transfer::staging::{BoundChunkReceipt, ChunkStaging, ChunkStagingError},
    file_upload_store::{
        FileUploadReservation, FileUploadStore, FileUploadStoreError, UploadReservationPin,
    },
};
use crate::models::collaboration::*;
use std::{fmt, io::Read, sync::Arc};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

mod journaled;
mod publication;
mod whole;
pub use journaled::{
    JournaledFileUploadStaging, JournaledUploadChunkAttempt, JournaledUploadChunkError,
};
pub use publication::{
    FileUploadPublicationAttempt, FileUploadPublicationError, FileUploadPublicationReceipt,
    FileUploadPublicationState, FileUploadPublisher,
};
pub use whole::{UploadFileObservation, UploadFileVerificationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadChunkState {
    Ready,
    Unknown,
    Confirmed,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UploadChunkError {
    #[error("invalid upload chunk input")]
    InvalidInput,
    #[error("upload chunk capacity exhausted")]
    CapacityExhausted,
    #[error("upload reservation not visible")]
    NotVisible,
    #[error("upload reservation storage rejected the operation: {0}")]
    Store(FileUploadStoreError),
    #[error("upload chunk already exists")]
    Conflict,
    #[error("upload chunk unavailable")]
    Unavailable,
    #[error("upload chunk outcome unconfirmed")]
    Unconfirmed,
}

struct Inner {
    store: FileUploadStore,
    staging: ChunkStaging,
    controls: Arc<Semaphore>,
}

#[derive(Clone)]
pub struct FileUploadStaging {
    inner: Arc<Inner>,
}

pub struct UploadChunkAttempt {
    issuer: Arc<Inner>,
    pin: UploadReservationPin,
    key: [u8; 32],
    index: u32,
    state: UploadChunkState,
}

impl UploadChunkAttempt {
    pub fn upload_id(&self) -> FileUploadId {
        self.pin.reservation().upload_id()
    }
    pub fn owner(&self) -> PrincipalRef {
        self.pin.reservation().owner()
    }
    pub fn account_id(&self) -> LegacyAccountId {
        self.pin.reservation().account_id()
    }
    pub fn manifest(&self) -> &FileTransferManifest {
        self.pin.reservation().manifest()
    }
    pub fn chunk_index(&self) -> u32 {
        self.index
    }
    pub fn state(&self) -> UploadChunkState {
        self.state
    }
}

pub struct UploadChunkReceipt {
    reservation: FileUploadReservation,
    index: u32,
    // Retains the original read-only descriptor. Neither metadata equality nor matching bytes at
    // a replacement pathname can manufacture this private in-process proof.
    _proof: BoundChunkReceipt,
}

impl UploadChunkReceipt {
    pub fn upload_id(&self) -> FileUploadId {
        self.reservation.upload_id()
    }
    pub fn owner(&self) -> PrincipalRef {
        self.reservation.owner()
    }
    pub fn account_id(&self) -> LegacyAccountId {
        self.reservation.account_id()
    }
    pub fn manifest(&self) -> &FileTransferManifest {
        self.reservation.manifest()
    }
    pub fn chunk_index(&self) -> u32 {
        self.index
    }
}

impl fmt::Debug for UploadChunkReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UploadChunkReceipt")
            .field("reservation", &self.reservation)
            .field("index", &self.index)
            .finish()
    }
}

impl FileUploadStaging {
    /// Freezes two trusted handles without database or filesystem I/O. Clones share admission and
    /// issuer identity; separately constructed adapters have additive waiting budgets.
    pub fn new(
        store: FileUploadStore,
        staging: ChunkStaging,
        controls: usize,
    ) -> Result<Self, UploadChunkError> {
        if !(1..=16).contains(&controls) || store.scope() != staging.scope() {
            return Err(UploadChunkError::InvalidInput);
        }
        Ok(Self {
            inner: Arc::new(Inner {
                store,
                staging,
                controls: Arc::new(Semaphore::new(controls)),
            }),
        })
    }

    fn admit(&self) -> Result<OwnedSemaphorePermit, UploadChunkError> {
        self.inner
            .controls
            .clone()
            .try_acquire_owned()
            .map_err(|_| UploadChunkError::CapacityExhausted)
    }

    /// Reads one exact reservation and validates its index. No current-root or chunk read occurs.
    pub async fn prepare(
        &self,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
        index: u32,
    ) -> Result<UploadChunkAttempt, UploadChunkError> {
        let _permit = self.admit()?;
        self.prepare_admitted(upload_id, owner, account_id, index)
            .await
    }

    // The journaled wrapper retains this same control permit through both metadata captures.
    // Callers acquire it before entering; this helper never installs, dispatches or adds admission.
    async fn prepare_admitted(
        &self,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
        index: u32,
    ) -> Result<UploadChunkAttempt, UploadChunkError> {
        let pin = self
            .inner
            .store
            .capture_pin(upload_id, owner, account_id)
            .await
            .map_err(UploadChunkError::Store)?
            .ok_or(UploadChunkError::NotVisible)?;
        pin.reservation()
            .manifest()
            .chunk(index)
            .map_err(|_| UploadChunkError::InvalidInput)?;
        let key = self.inner.staging.bind_upload(pin.source_key());
        Ok(UploadChunkAttempt {
            issuer: Arc::clone(&self.inner),
            pin,
            key,
            index,
            state: UploadChunkState::Ready,
        })
    }

    /// Only the issuing adapter or a clone accepts the attempt. Control rejection leaves Ready;
    /// every admitted outcome is terminal for this attempt, including cancellation and known errors.
    /// Controls bound live waiters, not cancelled blocking work; staging jobs retain worker permits.
    pub async fn stage_chunk<R: Read + Send + 'static>(
        &self,
        attempt: &mut UploadChunkAttempt,
        reader: R,
    ) -> Result<UploadChunkReceipt, UploadChunkError> {
        if !Arc::ptr_eq(&self.inner, &attempt.issuer) || attempt.state != UploadChunkState::Ready {
            return Err(UploadChunkError::InvalidInput);
        }
        let _permit = self.admit()?;
        // This precedes the first await, including a pre-write store validation which may fail.
        attempt.state = UploadChunkState::Unknown;
        self.inner
            .store
            .verify_pin(&attempt.pin)
            .await
            .map_err(UploadChunkError::Store)?;
        let proof = self
            .inner
            .staging
            .stage_bound_chunk(attempt.key, attempt.manifest(), attempt.index, reader)
            .await
            .map_err(stage_error)?;
        self.inner
            .store
            .verify_pin(&attempt.pin)
            .await
            .map_err(|_| UploadChunkError::Unconfirmed)?;
        let proof = self
            .inner
            .staging
            .verify_bound_receipt(attempt.key, attempt.manifest(), attempt.index, proof)
            .await
            .map_err(|_| UploadChunkError::Unconfirmed)?;
        let receipt = UploadChunkReceipt {
            reservation: attempt.pin.reservation().clone(),
            index: attempt.index,
            _proof: proof,
        };
        attempt.state = UploadChunkState::Confirmed;
        Ok(receipt)
    }
}

fn stage_error(error: ChunkStagingError) -> UploadChunkError {
    match error {
        ChunkStagingError::CapacityExhausted => UploadChunkError::CapacityExhausted,
        ChunkStagingError::Conflict => UploadChunkError::Conflict,
        ChunkStagingError::Unavailable => UploadChunkError::Unavailable,
        ChunkStagingError::InvalidInput | ChunkStagingError::Unconfirmed => {
            UploadChunkError::Unconfirmed
        }
    }
}
