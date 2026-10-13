//! Assembly uses the existing job-only budget; cancellation never releases a real job's slot.
use super::{assemble, work, AssembledFile, ChunkStagingError, FileTransferManifest, Inner};
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;

pub(super) async fn run(
    inner: &Arc<Inner>,
    manifest: &FileTransferManifest,
    flow: Option<Arc<OwnedSemaphorePermit>>,
) -> Result<AssembledFile, ChunkStagingError> {
    let permit = work::admit(inner)?;
    let manifest = manifest.clone();
    let inner = Arc::clone(inner);
    tokio::task::spawn_blocking(move || {
        let _flow = flow;
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
    run_with_flow(inner, manifest, observer, None).await
}

#[cfg(test)]
pub(super) async fn run_with_flow<O: FnMut(assemble::Phase) + Send + 'static>(
    inner: &Arc<Inner>,
    manifest: &FileTransferManifest,
    observer: O,
    flow: Option<Arc<OwnedSemaphorePermit>>,
) -> Result<AssembledFile, ChunkStagingError> {
    let permit = work::admit(inner)?;
    let manifest = manifest.clone();
    let inner = Arc::clone(inner);
    tokio::task::spawn_blocking(move || {
        let _flow = flow;
        let _permit = permit;
        assemble::run_with(&inner.directory, &manifest, observer)
    })
    .await
    .map_err(|_| ChunkStagingError::Unconfirmed)?
}
