//! Bound operations use the same fail-fast gate as legacy staging; jobs own their permits.
use super::{bound, work, BoundChunkReceipt, ChunkStagingError, FileTransferManifest, Inner};
use std::{io::Read, sync::Arc};

pub(super) enum Operation<R> {
    Stage(R),
    Verify(BoundChunkReceipt),
}

pub(super) async fn run<R: Read + Send + 'static>(
    inner: &Arc<Inner>,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    index: u32,
    operation: Operation<R>,
) -> Result<BoundChunkReceipt, ChunkStagingError> {
    let permit = work::admit(inner)?;
    // Shared admission precedes metadata cloning, and queue/cancellation cannot release the slot.
    let manifest = manifest.clone();
    let inner = Arc::clone(inner);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        match operation {
            Operation::Stage(reader) => {
                bound::stage(&inner.directory, key, &manifest, index, reader)
            }
            Operation::Verify(receipt) => {
                bound::verify(&inner.directory, key, &manifest, index, receipt)
            }
        }
    })
    .await
    .map_err(|_| ChunkStagingError::Unconfirmed)?
}
