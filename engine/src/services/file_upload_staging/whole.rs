//! Read-only current-byte observations, never publication, receipt recovery or transfer authority.
use super::{FileUploadReservation, FileUploadStaging, FileUploadStoreError};
use crate::models::collaboration::{
    FileTransferManifest, FileUploadId, LegacyAccountId, PrincipalRef,
};
use crate::services::file_transfer::staging::ChunkStagingError;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UploadFileVerificationError {
    #[error("upload file verification capacity exhausted")]
    CapacityExhausted,
    #[error("upload reservation not visible")]
    NotVisible,
    #[error("upload reservation storage rejected verification: {0}")]
    Store(FileUploadStoreError),
    #[error("upload file verification unavailable")]
    Unavailable,
    #[error("upload file observation unconfirmed")]
    Unconfirmed,
}

/// Supplied metadata for one current-byte observation, not a reusable grant or original write proof.
#[derive(Debug)]
pub struct UploadFileObservation {
    reservation: FileUploadReservation,
}

impl UploadFileObservation {
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
}

impl FileUploadStaging {
    /// Observes the exact upload-bound current chunks only. Does not install, write or recover state.
    pub async fn verify_file(
        &self,
        upload_id: FileUploadId,
        owner: PrincipalRef,
        account_id: LegacyAccountId,
    ) -> Result<UploadFileObservation, UploadFileVerificationError> {
        // Controls bound this live caller, while each queued/running filesystem job owns its
        // separate original staging worker permit even when the caller is cancelled.
        let _permit = self
            .admit()
            .map_err(|_| UploadFileVerificationError::CapacityExhausted)?;
        let pin = self
            .inner
            .store
            .capture_pin(upload_id, owner, account_id)
            .await
            .map_err(UploadFileVerificationError::Store)?
            .ok_or(UploadFileVerificationError::NotVisible)?;
        let key = self.inner.staging.bind_upload(pin.source_key());
        let manifest = pin.reservation().manifest();
        let observation = self
            .inner
            .staging
            .observe_bound_file(key, manifest)
            .await
            .map_err(initial_filesystem_error)?;
        // This is a fresh observation of the original source, not a transaction spanning files.
        self.inner
            .store
            .verify_pin(&pin)
            .await
            .map_err(|_| UploadFileVerificationError::Unconfirmed)?;
        self.inner
            .staging
            .recheck_bound_file(key, manifest, observation)
            .await
            .map_err(|_| UploadFileVerificationError::Unconfirmed)?;
        // No awaited operation follows the final root/entry snapshots. Metadata cannot recreate
        // a chunk receipt or guarantee permanent immutability after these separate observations.
        Ok(UploadFileObservation {
            reservation: pin.reservation().clone(),
        })
    }
}

fn initial_filesystem_error(error: ChunkStagingError) -> UploadFileVerificationError {
    match error {
        ChunkStagingError::CapacityExhausted => UploadFileVerificationError::CapacityExhausted,
        ChunkStagingError::Unconfirmed => UploadFileVerificationError::Unconfirmed,
        _ => UploadFileVerificationError::Unavailable,
    }
}
