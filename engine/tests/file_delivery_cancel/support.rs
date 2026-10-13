// Only prior pure assertion/transport helpers are nested, never their test cases.
#[allow(dead_code)]
#[path = "../file_delivery_progress/support.rs"]
mod observation;
use crate::{fixture::*, *};
pub use observation::{
    assert_output, changed, chunk_error, closed, drive, indices, observed, plain, snapshot,
    staging_conflict, verify_prefix, within,
};

pub async fn controlled(
    f: &Fixture,
    client: &FileChunkClient,
    target: &FileSaveTarget,
    observer: FileDeliveryObserver,
    cancellation: FileDeliveryCancellation,
) -> Result<FileVersionRef, FileControlledSaveError> {
    within(client.fetch_granted_and_save_controlled(
        TOKEN,
        grant(),
        upload(),
        &FileVersionRef::from_manifest(&f.manifest),
        target,
        observer,
        cancellation,
    ))
    .await
    .map(|saved| saved.file().clone())
}
pub fn wrapped(error: FileGrantedSaveError) -> FileControlledSaveError {
    FileControlledSaveError::Delivery(error)
}
pub fn assert_cancelled(error: FileControlledSaveError) {
    assert_eq!(error, FileControlledSaveError::Cancelled);
}
