//! The original source proof, destination and worker slot remain owned by the actual blocking job.
use super::{
    assembled_proof::AssembledProof, save::Target, save_io, work, FileSaveError,
    FileTransferManifest, Inner, SavedFile,
};
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;

pub(super) async fn run(
    inner: &Arc<Inner>,
    manifest: &FileTransferManifest,
    proof: &Arc<AssembledProof>,
    target: &Arc<Target>,
    flow: Option<Arc<OwnedSemaphorePermit>>,
) -> Result<SavedFile, FileSaveError> {
    let permit = work::admit(inner).map_err(|_| FileSaveError::CapacityExhausted)?;
    let inner = Arc::clone(inner);
    let manifest = manifest.clone();
    let proof = Arc::clone(proof);
    let target = Arc::clone(target);
    #[cfg(test)]
    if let Some(observer) = &target.observer {
        observer(super::save::SaveTestPhase::BeforeDispatch);
    }
    tokio::task::spawn_blocking(move || {
        let _flow = flow;
        let _permit = permit;
        #[cfg(test)]
        if let Some(observer) = &target.observer {
            return save_io::run_with(&inner.directory, &manifest, &proof, &target, |phase| {
                use super::save::SaveTestPhase;
                observer(match phase {
                    save_io::Phase::Created => SaveTestPhase::Created,
                    save_io::Phase::Copied => SaveTestPhase::Copied,
                    save_io::Phase::Readback => SaveTestPhase::Readback,
                });
            });
        }
        save_io::run(&inner.directory, &manifest, &proof, &target)
    })
    .await
    .map_err(|_| FileSaveError::Unconfirmed)?
}

#[cfg(test)]
pub(super) async fn run_with<O: FnMut(save_io::Phase) + Send + 'static>(
    inner: &Arc<Inner>,
    manifest: &FileTransferManifest,
    proof: &Arc<AssembledProof>,
    target: &Arc<Target>,
    observer: O,
) -> Result<SavedFile, FileSaveError> {
    let permit = work::admit(inner).map_err(|_| FileSaveError::CapacityExhausted)?;
    let inner = Arc::clone(inner);
    let manifest = manifest.clone();
    let proof = Arc::clone(proof);
    let target = Arc::clone(target);
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        save_io::run_with(&inner.directory, &manifest, &proof, &target, observer)
    })
    .await
    .map_err(|_| FileSaveError::Unconfirmed)?
}
