//! Whole-file jobs share the existing staging admission budget and own permits through completion.
use super::{whole, work, BoundFileObservation, ChunkStagingError, FileTransferManifest, Inner};
use std::sync::Arc;

pub(super) async fn observe(
    inner: &Arc<Inner>,
    key: [u8; 32],
    manifest: &FileTransferManifest,
) -> Result<BoundFileObservation, ChunkStagingError> {
    let permit = work::admit(inner)?;
    let manifest = manifest.clone();
    let inner = Arc::clone(inner);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        whole::observe(&inner.directory, key, &manifest)
    })
    .await
    .map_err(|_| ChunkStagingError::Unconfirmed)?
}

pub(super) async fn recheck(
    inner: &Arc<Inner>,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    observation: BoundFileObservation,
) -> Result<(), ChunkStagingError> {
    let permit = work::admit(inner)?;
    let manifest = manifest.clone();
    let inner = Arc::clone(inner);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        whole::recheck(&inner.directory, key, &manifest, observation)
    })
    .await
    .map_err(|_| ChunkStagingError::Unconfirmed)?
}

pub(super) async fn recheck_retained(
    inner: &Arc<Inner>,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    observation: BoundFileObservation,
) -> Result<BoundFileObservation, ChunkStagingError> {
    let permit = work::admit(inner)?;
    let manifest = manifest.clone();
    let inner = Arc::clone(inner);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        whole::recheck_retained(&inner.directory, key, &manifest, observation)
    })
    .await
    .map_err(|_| ChunkStagingError::Unconfirmed)?
}

#[cfg(test)]
pub(super) async fn observe_with<O: FnMut(u32) + Send + 'static>(
    inner: &Arc<Inner>,
    key: [u8; 32],
    manifest: &FileTransferManifest,
    observer: O,
) -> Result<BoundFileObservation, ChunkStagingError> {
    let permit = work::admit(inner)?;
    let manifest = manifest.clone();
    let inner = Arc::clone(inner);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        whole::observe_with(&inner.directory, key, &manifest, observer)
    })
    .await
    .map_err(|_| ChunkStagingError::Unconfirmed)?
}
