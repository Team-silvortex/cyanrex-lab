use super::{JournaledFileUploadStaging, JournaledUploadChunkAttempt, JournaledUploadChunkError};
use crate::services::{
    file_transfer::staging::{BoundChunkReceipt, ChunkStagingError},
    file_upload_staging::{stage_error, UploadChunkError},
};

impl JournaledFileUploadStaging {
    pub(super) async fn verify_source(
        &self,
        attempt: &JournaledUploadChunkAttempt,
    ) -> Result<(), UploadChunkError> {
        self.inner
            .local
            .inner
            .store
            .verify_pin(&attempt.local.pin)
            .await
            .map_err(UploadChunkError::Store)
    }

    pub(super) async fn verify_local_proof(
        &self,
        attempt: &JournaledUploadChunkAttempt,
        proof: BoundChunkReceipt,
    ) -> Result<BoundChunkReceipt, JournaledUploadChunkError> {
        self.verify_source(attempt)
            .await
            .map_err(|_| JournaledUploadChunkError::Unconfirmed)?;
        self.inner
            .local
            .inner
            .staging
            .verify_bound_receipt(
                attempt.local.key,
                attempt.manifest(),
                attempt.chunk_index(),
                proof,
            )
            .await
            .map_err(|_| JournaledUploadChunkError::Unconfirmed)
    }
}

pub(super) fn initial_stage_error(error: ChunkStagingError) -> JournaledUploadChunkError {
    match stage_error(error) {
        UploadChunkError::Unconfirmed => JournaledUploadChunkError::Unconfirmed,
        known => JournaledUploadChunkError::Local(known),
    }
}
