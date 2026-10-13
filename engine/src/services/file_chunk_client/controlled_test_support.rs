pub(super) use super::super::{
    download_test_support::{
        client, ids, manifest, name, restored, verify_output, Barrier, ReleaseOnDrop, Root, TOKEN,
    },
    manifest_work_support::Server,
    FileDeliveryCancelHandle, FileDeliveryPhase, FileDeliveryProgress, FileDeliverySnapshot,
    FileDownloadError, FileGrantedDownloadError,
};
use std::{fs, os::unix::fs::MetadataExt, path::Path, time::Duration};
use tokio::time::timeout;

pub(super) async fn verify_after_worker_drain(
    client: &super::super::FileChunkClient,
    manifest: &crate::models::collaboration::FileTransferManifest,
    index: u32,
) -> Result<(), crate::services::file_transfer::staging::ChunkStagingError> {
    use crate::services::file_transfer::staging::ChunkStagingError;
    // A leased reader can release the flow before the enclosing stage closure drops its worker.
    // Wait only for that job tail, by executing a real read-only verifier, not a fabricated ACK.
    // Any result other than temporary admission saturation is returned unchanged and asserted.
    timeout(Duration::from_secs(10), async {
        loop {
            match client
                .inner
                .staging
                .verify_staged_chunk(manifest, index)
                .await
            {
                Err(ChunkStagingError::CapacityExhausted) => tokio::task::yield_now().await,
                result => return result,
            }
        }
    })
    .await
    .expect("the original file worker must actually finish within the bounded drain")
}

pub(super) fn prefix(
    progress: &FileDeliveryProgress,
    phase: FileDeliveryPhase,
    chunks: u32,
    bytes: u64,
    total_bytes: u64,
    total_chunks: Option<u32>,
) -> FileDeliverySnapshot {
    let snapshot = progress
        .latest()
        .expect("this actually admitted call has an observation");
    assert_eq!(snapshot.phase(), phase);
    assert_eq!(snapshot.staged_chunks(), chunks);
    assert_eq!(snapshot.staged_bytes(), bytes);
    assert_eq!(snapshot.total_bytes(), total_bytes);
    assert_eq!(snapshot.total_chunks(), total_chunks);
    snapshot
}

pub(super) async fn closed(progress: &mut FileDeliveryProgress, last: FileDeliverySnapshot) {
    // Only one unseen latest value can precede closure; this is not an event queue or job drain.
    if let Some(snapshot) = timeout(Duration::from_secs(10), progress.changed())
        .await
        .unwrap()
    {
        assert_eq!(snapshot, last);
        assert_eq!(
            timeout(Duration::from_secs(10), progress.changed())
                .await
                .unwrap(),
            None
        );
    }
    assert_eq!(progress.latest(), Some(last));
}

type Stamp = (u64, u64, u32, u32, u64, u64, i64, i64, i64, i64);
pub(super) fn tree(path: &Path) -> Vec<(String, Stamp, Vec<u8>)> {
    let mut entries = fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let metadata = entry.metadata().unwrap();
            (
                entry.file_name().to_str().unwrap().to_owned(),
                (
                    metadata.dev(),
                    metadata.ino(),
                    metadata.mode(),
                    metadata.uid(),
                    metadata.nlink(),
                    metadata.len(),
                    metadata.mtime(),
                    metadata.mtime_nsec(),
                    metadata.ctime(),
                    metadata.ctime_nsec(),
                ),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}
