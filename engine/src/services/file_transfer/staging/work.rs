//! One blocking dispatch point; permits belong to jobs, not their cancellable waiters.
use super::{ChunkStagingError, FileTransferManifest, Inner};
use std::{io::Read, sync::Arc};
use tokio::sync::OwnedSemaphorePermit;

pub(super) enum Operation<R> {
    Stage(R),
    Verify,
}

struct JobPermit {
    _permit: OwnedSemaphorePermit,
}

pub(super) fn admit(inner: &Arc<Inner>) -> Result<OwnedSemaphorePermit, ChunkStagingError> {
    inner
        .workers
        .clone()
        .try_acquire_owned()
        .map_err(|_| ChunkStagingError::CapacityExhausted)
}

pub(super) async fn run<R: Read + Send + 'static>(
    inner: &Arc<Inner>,
    manifest: &FileTransferManifest,
    index: u32,
    operation: Operation<R>,
) -> Result<(), ChunkStagingError> {
    let permit = JobPermit {
        _permit: admit(inner)?,
    };
    // Cloning bounded metadata and submitting work both happen only after fail-fast admission.
    let manifest = manifest.clone();
    let inner = Arc::clone(inner);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        match operation {
            Operation::Stage(reader) => inner.directory.stage(&manifest, index, reader),
            Operation::Verify => inner
                .directory
                .verify_chunk(&manifest, index)
                .map_err(|_| ChunkStagingError::Unavailable),
        }
    })
    .await
    .map_err(|_| ChunkStagingError::Unconfirmed)?
}
