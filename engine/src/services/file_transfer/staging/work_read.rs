//! Buffered reads share the existing staging job budget; disclosure remains caller-owned.
use super::{read, work, BufferedBoundChunkRead, ChunkStagingError, FileTransferManifest, Inner};
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;

pub(super) async fn run(
    inner: &Arc<Inner>,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    index: u32,
    lease: OwnedSemaphorePermit,
) -> Result<BufferedBoundChunkRead, ChunkStagingError> {
    run_inner(inner, key, manifest, index, lease, |_| {}).await
}

async fn run_inner<O: FnMut(read::ReadPoint) + Send + 'static>(
    inner: &Arc<Inner>,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    index: u32,
    lease: OwnedSemaphorePermit,
    observer: O,
) -> Result<BufferedBoundChunkRead, ChunkStagingError> {
    let permit = work::admit(inner)?;
    // Admission precedes bounded metadata cloning and submission. Both the actual worker slot
    // and the independent byte lease belong to queued/running work, not its cancellable waiter.
    let manifest = manifest.clone();
    let inner = Arc::clone(inner);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        // Success moves the byte lease into the result, including an unclaimed completed result.
        read::read(&inner.directory, key, &manifest, index, lease, observer)
    })
    .await
    .map_err(|_| ChunkStagingError::Unconfirmed)?
}

#[cfg(test)]
pub(super) async fn run_with<O: FnMut(read::ReadPoint) + Send + 'static>(
    inner: &Arc<Inner>,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    index: u32,
    lease: OwnedSemaphorePermit,
    observer: O,
) -> Result<BufferedBoundChunkRead, ChunkStagingError> {
    run_inner(inner, key, manifest, index, lease, observer).await
}
