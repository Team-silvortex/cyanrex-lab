//! Controlled waiting only; no extra file worker, task, callback, retry or cleanup.
use super::{
    delivery, FileDeliveryCancellation, FileDeliveryObserver, FileGrantId, FileGrantedSaveError,
    FileSaveTarget, FileUploadId, FileVersionRef, Inner, SavedFile,
};
use std::future::Future;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FileControlledSaveError {
    #[error("file delivery waiting cancelled")]
    Cancelled,
    #[error(transparent)]
    Delivery(FileGrantedSaveError),
}

pub(super) async fn run(
    inner: &Inner,
    token: &str,
    grant_id: FileGrantId,
    upload_id: FileUploadId,
    expected_file: &FileVersionRef,
    target: &FileSaveTarget,
    observer: FileDeliveryObserver,
    cancellation: FileDeliveryCancellation,
) -> Result<SavedFile, FileControlledSaveError> {
    // Invalid trusted inputs win even if cancellation was requested or the whole-flow gate is full.
    delivery::validate_inputs(inner, token, expected_file, target)
        .map_err(FileControlledSaveError::Delivery)?;
    wait_for_delivery(
        cancellation,
        delivery::run_observed(
            inner,
            token,
            grant_id,
            upload_id,
            expected_file,
            target,
            observer,
        ),
    )
    .await
}

async fn wait_for_delivery<T, F>(
    cancellation: FileDeliveryCancellation,
    delivery: F,
) -> Result<T, FileControlledSaveError>
where
    F: Future<Output = Result<T, FileGrantedSaveError>>,
{
    // Priority applies to readiness at this poll, not wall-clock preemption of synchronous work.
    // Dropping the losing live future ends its observer and scheduling; actual jobs retain leases.
    tokio::select! {
        biased;
        _ = cancellation.requested() => Err(FileControlledSaveError::Cancelled),
        result = delivery => result.map_err(FileControlledSaveError::Delivery),
    }
}

#[cfg(test)]
#[path = "controlled_channel_tests.rs"]
mod channel_tests;
#[cfg(test)]
#[path = "controlled_file_tests.rs"]
mod file_tests;
#[cfg(test)]
#[path = "controlled_test_support.rs"]
mod test_support;
#[cfg(test)]
#[path = "controlled_work_tests.rs"]
mod work_tests;
