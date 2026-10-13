//! One live download flow through explicit private saving; no detached continuation or recovery.
use super::{
    download, manifest, manifest_inputs_valid, FileDeliveryObserver, FileDeliveryPhase,
    FileGrantId, FileGrantedDownloadError, FileGrantedSaveError, FileSaveTarget, FileUploadId,
    FileVersionRef, Inner, SavedFile,
};
use std::sync::Arc;

pub(super) fn validate_inputs(
    inner: &Inner,
    token: &str,
    expected_file: &FileVersionRef,
    target: &FileSaveTarget,
) -> Result<(), FileGrantedSaveError> {
    // Cached root comparison only: no current-path IO, target reservation or remote-name policy.
    if !manifest_inputs_valid(inner, token, expected_file)
        || inner.staging.validate_save_target(target).is_err()
    {
        return Err(FileGrantedSaveError::InvalidInput);
    }
    Ok(())
}

pub(super) async fn run(
    inner: &Inner,
    token: &str,
    grant_id: FileGrantId,
    upload_id: FileUploadId,
    expected_file: &FileVersionRef,
    target: &FileSaveTarget,
) -> Result<SavedFile, FileGrantedSaveError> {
    run_inner(
        inner,
        token,
        grant_id,
        upload_id,
        expected_file,
        target,
        None,
    )
    .await
}

pub(super) async fn run_observed(
    inner: &Inner,
    token: &str,
    grant_id: FileGrantId,
    upload_id: FileUploadId,
    expected_file: &FileVersionRef,
    target: &FileSaveTarget,
    observer: FileDeliveryObserver,
) -> Result<SavedFile, FileGrantedSaveError> {
    run_inner(
        inner,
        token,
        grant_id,
        upload_id,
        expected_file,
        target,
        Some(observer),
    )
    .await
}

async fn run_inner(
    inner: &Inner,
    token: &str,
    grant_id: FileGrantId,
    upload_id: FileUploadId,
    expected_file: &FileVersionRef,
    target: &FileSaveTarget,
    observer: Option<FileDeliveryObserver>,
) -> Result<SavedFile, FileGrantedSaveError> {
    validate_inputs(inner, token, expected_file, target)?;
    let flow = Arc::new(
        inner
            .downloads
            .clone()
            .try_acquire_owned()
            .map_err(|_| FileGrantedSaveError::CapacityExhausted)?,
    );
    if let Some(observer) = &observer {
        observer.report(
            FileDeliveryPhase::FetchingManifest,
            0,
            0,
            expected_file.byte_length(),
            None,
        );
    }
    let manifest = manifest::fetch(inner, token, grant_id, upload_id, expected_file)
        .await
        .map_err(|error| {
            FileGrantedSaveError::Download(FileGrantedDownloadError::Manifest(error))
        })?;
    let total_chunks = manifest.chunk_sha256().len() as u32;
    if let Some(observer) = &observer {
        observer.report(
            FileDeliveryPhase::StagingChunks,
            0,
            0,
            manifest.byte_length(),
            Some(total_chunks),
        );
    }
    // Retain the parent flow while real per-chunk and assembly jobs own their own references.
    let assembled = download::run_with_flow_observed(
        inner,
        token,
        grant_id,
        upload_id,
        &manifest,
        Arc::clone(&flow),
        observer.as_ref(),
    )
    .await
    .map_err(|error| FileGrantedSaveError::Download(FileGrantedDownloadError::Download(error)))?;
    if let Some(observer) = &observer {
        observer.report(
            FileDeliveryPhase::Saving,
            total_chunks,
            manifest.byte_length(),
            manifest.byte_length(),
            Some(total_chunks),
        );
    }
    // This original ACK, not metadata-reopened proof, crosses into the actual save closure.
    let saved = inner
        .staging
        .save_assembled_file_leased(&manifest, &assembled, target, Some(flow))
        .await
        .map_err(FileGrantedSaveError::Save)?;
    if let Some(observer) = &observer {
        observer.report(
            FileDeliveryPhase::Complete,
            total_chunks,
            manifest.byte_length(),
            manifest.byte_length(),
            Some(total_chunks),
        );
    }
    Ok(saved)
}

#[cfg(test)]
#[path = "delivery_save_tests.rs"]
mod save_tests;
#[cfg(test)]
#[path = "delivery_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "delivery_user_save_tests.rs"]
mod user_save_tests;
#[cfg(test)]
#[path = "delivery_work_tests.rs"]
mod work_tests;
