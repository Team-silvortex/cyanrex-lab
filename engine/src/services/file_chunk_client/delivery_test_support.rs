//! Private actual-file oracles; production clients cannot supply bodies or phase callbacks.
pub(super) use super::super::{
    download_test_support::{
        client, ids, manifest, name, restored, verify_output, Barrier, ReleaseOnDrop, Root, TOKEN,
    },
    manifest_work_support::Server,
    FileChunkClient, FileDownloadError, FileGrantedDownloadError, FileGrantedSaveError,
    FileSaveTarget, FileVersionRef,
};
use crate::services::file_transfer::verify_file;
use std::{fs, os::unix::fs::MetadataExt, path::Path, time::Duration};

pub(super) const LEAF: &str = "host-selected.bin";

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Stamp(u64, u64, u32, u32, u64, u64, i64, i64, i64, i64);
impl Stamp {
    fn of(meta: &fs::Metadata) -> Self {
        Self(
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
        )
    }
}
pub(super) fn tree(root: &Path) -> Vec<(String, Stamp, Vec<u8>)> {
    let mut entries = fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_str().unwrap().to_owned(),
                Stamp::of(&entry.metadata().unwrap()),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    entries
}

pub(super) fn saved(
    source: &Root,
    destination: &Root,
    manifest: &super::super::FileTransferManifest,
    bytes: &[u8],
    original_inode: Option<u64>,
) {
    let path = destination.0.join(LEAF);
    let metadata = fs::symlink_metadata(&path).unwrap();
    assert!(metadata.is_file());
    assert_eq!(metadata.len(), bytes.len() as u64);
    assert_eq!(metadata.mode() & 0o7777, 0o400);
    assert_eq!(metadata.uid(), fs::metadata(&destination.0).unwrap().uid());
    assert_eq!(metadata.nlink(), 1);
    if let Some(inode) = original_inode {
        assert_eq!(metadata.ino(), inode);
    }
    let assembled = fs::metadata(source.0.join(name(manifest, "f1", None))).unwrap();
    assert_ne!(
        (metadata.dev(), metadata.ino()),
        (assembled.dev(), assembled.ino())
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    verify_file(manifest, &mut fs::File::open(path).unwrap()).unwrap();
    assert_eq!(fs::read_dir(&destination.0).unwrap().count(), 1);
    source.unchanged();
    destination.unchanged();
}

pub(super) async fn positive(
    origin: &str,
    source: &Root,
    destination: &Root,
    manifest: &super::super::FileTransferManifest,
    bytes: &[u8],
) {
    let client = client(origin, source, manifest);
    let target = FileSaveTarget::open(&destination.0, LEAF).unwrap();
    let (grant, upload) = ids();
    let expected = FileVersionRef::from_manifest(manifest);
    let ack = tokio::time::timeout(
        Duration::from_secs(10),
        client.fetch_granted_and_save(TOKEN, grant, upload, &expected, &target),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(ack.file(), &expected);
    // The public metadata result holds neither a worker nor the whole-download flow slot.
    assert_eq!(client.inner.downloads.available_permits(), 1);
    assert_eq!(client.inner.manifests.available_permits(), 1);
    assert_eq!(client.inner.controls.available_permits(), 1);
    verify_output(source, manifest, bytes);
    saved(source, destination, manifest, bytes, None);
}
