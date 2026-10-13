//! Ordered live calls only: no detached continuation, saved progress, retry or entry adoption.
use super::{
    request, AssembledFile, FileDeliveryObserver, FileDeliveryPhase, FileDownloadError,
    FileGrantId, FileTransferManifest, FileUploadId, Inner,
};
use std::sync::Arc;
use tokio::sync::OwnedSemaphorePermit;

pub(super) async fn run(
    inner: &Inner,
    token: &str,
    grant_id: FileGrantId,
    upload_id: FileUploadId,
    manifest: &FileTransferManifest,
) -> Result<AssembledFile, FileDownloadError> {
    let flow = Arc::new(
        inner
            .downloads
            .clone()
            .try_acquire_owned()
            .map_err(|_| FileDownloadError::CapacityExhausted)?,
    );
    run_with_flow(inner, token, grant_id, upload_id, manifest, flow).await
}

/// One admitted parent flow is reused by both supplied and remotely observed manifests.
pub(super) async fn run_with_flow(
    inner: &Inner,
    token: &str,
    grant_id: FileGrantId,
    upload_id: FileUploadId,
    manifest: &FileTransferManifest,
    flow: Arc<OwnedSemaphorePermit>,
) -> Result<AssembledFile, FileDownloadError> {
    run_with_flow_observed(inner, token, grant_id, upload_id, manifest, flow, None).await
}

pub(super) async fn run_with_flow_observed(
    inner: &Inner,
    token: &str,
    grant_id: FileGrantId,
    upload_id: FileUploadId,
    manifest: &FileTransferManifest,
    flow: Arc<OwnedSemaphorePermit>,
    observer: Option<&FileDeliveryObserver>,
) -> Result<AssembledFile, FileDownloadError> {
    let mut staged_bytes = 0;
    for index in 0..manifest.chunk_sha256().len() as u32 {
        // Acknowledged stage completion precedes the next request. Metadata is not accumulated
        // as recoverable progress, and every existing entry still conflicts in the old stage.
        let staged = request::fetch_with_flow(
            inner,
            token,
            grant_id,
            upload_id,
            manifest,
            index,
            Some(flow.clone()),
        )
        .await
        .map_err(|error| FileDownloadError::Chunk { index, error })?;
        if let Some(observer) = observer {
            // The received opaque local ACK, not a response prefix or guessed file, advances counters.
            // Exact typed chunk lengths sum to the manifest's <=64 GiB, at most 16,384 chunks.
            staged_bytes += staged.byte_length();
            observer.report(
                FileDeliveryPhase::StagingChunks,
                index + 1,
                staged_bytes,
                manifest.byte_length(),
                Some(manifest.chunk_sha256().len() as u32),
            );
        }
    }
    if let Some(observer) = observer {
        observer.report(
            FileDeliveryPhase::Assembling,
            manifest.chunk_sha256().len() as u32,
            manifest.byte_length(),
            manifest.byte_length(),
            Some(manifest.chunk_sha256().len() as u32),
        );
    }
    // This core performs no chunk HTTP for empty manifests. The granted entry already completed
    // its metadata GET; the original supplied-manifest entry still establishes no remote authority.
    // The last parent reference enters the real blocking assembly job, not just its waiter.
    inner
        .staging
        .assemble_staged_file_leased(manifest, Some(flow))
        .await
        .map_err(FileDownloadError::Assembly)
}
