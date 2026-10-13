pub(super) use super::super::{
    download_test_support::{
        client, ids, manifest, name, restored, verify_output, Barrier, ReleaseOnDrop, Root, TOKEN,
    },
    manifest_work_support::Server,
    FileDownloadError, FileGrantedDownloadError, FileGrantedSaveError, FileSaveTarget,
    FileVersionRef,
};
use super::*;
use std::{fs, os::unix::fs::MetadataExt, path::Path, time::Duration};
use tokio::time::timeout;

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
        .expect("a real admitted observation must exist");
    assert_eq!(snapshot.phase(), phase);
    assert_eq!(snapshot.staged_chunks(), chunks);
    assert_eq!(snapshot.staged_bytes(), bytes);
    assert_eq!(snapshot.total_bytes(), total_bytes);
    assert_eq!(snapshot.total_chunks(), total_chunks);
    snapshot
}

pub(super) async fn closed(progress: &mut FileDeliveryProgress, last: FileDeliverySnapshot) {
    // The orchestrator is gone even while its old OS job is held. Consume an unseen final value
    // without relying on closure to overwrite it or treating closure as filesystem rollback.
    if let Some(snapshot) = timeout(Duration::from_secs(10), progress.changed())
        .await
        .unwrap()
    {
        assert_eq!(snapshot, last);
        assert_eq!(
            timeout(Duration::from_secs(10), progress.changed())
                .await
                .unwrap(),
            None,
            "one latest slot permits at most one unseen value followed by closure"
        );
    }
    assert_eq!(progress.latest(), Some(last));
}

pub(super) async fn drain_complete(progress: &mut FileDeliveryProgress, bytes: u64, chunks: u32) {
    let last = prefix(
        progress,
        FileDeliveryPhase::Complete,
        chunks,
        bytes,
        bytes,
        Some(chunks),
    );
    closed(progress, last).await;
}

type Stamp = (u64, u64, u32, u32, u64, u64, i64, i64, i64, i64);
pub(super) fn tree(path: &Path) -> Vec<(String, Stamp, Vec<u8>)> {
    let mut entries = fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let meta = entry.metadata().unwrap();
            (
                entry.file_name().to_str().unwrap().to_owned(),
                (
                    meta.dev(),
                    meta.ino(),
                    meta.mode(),
                    meta.uid(),
                    meta.nlink(),
                    meta.len(),
                    meta.mtime(),
                    meta.mtime_nsec(),
                    meta.ctime(),
                    meta.ctime_nsec(),
                ),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}
