//! Assembly uses the existing job-only budget; cancellation never releases a real job's slot.
use super::{assemble, work, AssembledFile, ChunkStagingError, FileTransferManifest, Inner};
use std::sync::Arc;

pub(super) async fn run(
    inner: &Arc<Inner>,
    manifest: &FileTransferManifest,
) -> Result<AssembledFile, ChunkStagingError> {
    let permit = work::admit(inner)?;
    let manifest = manifest.clone();
    let inner = Arc::clone(inner);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        assemble::run(&inner.directory, &manifest)
    })
    .await
    .map_err(|_| ChunkStagingError::Unconfirmed)?
}

#[cfg(test)]
pub(super) async fn run_with<O: FnMut(assemble::Phase) + Send + 'static>(
    inner: &Arc<Inner>,
    manifest: &FileTransferManifest,
    observer: O,
) -> Result<AssembledFile, ChunkStagingError> {
    let permit = work::admit(inner)?;
    let manifest = manifest.clone();
    let inner = Arc::clone(inner);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        assemble::run_with(&inner.directory, &manifest, observer)
    })
    .await
    .map_err(|_| ChunkStagingError::Unconfirmed)?
}
