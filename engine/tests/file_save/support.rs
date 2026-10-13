use crate::{fixture::*, *};
use std::{
    fs::{self, OpenOptions},
    os::unix::fs::{DirBuilderExt, FileTypeExt, OpenOptionsExt, PermissionsExt},
    path::Path,
    time::Duration,
};

pub async fn save(
    f: &Fixture,
    assembled: &AssembledFile,
    target: &FileSaveTarget,
) -> Result<FileVersionRef, FileSaveError> {
    within(f.store.save_assembled_file(&f.manifest, assembled, target))
        .await
        .map(|saved| saved.file().clone())
}
pub fn assert_output(f: &Fixture, path: &Path, manifest: &FileTransferManifest) -> (u64, u64) {
    let observed = observe(path, manifest).unwrap();
    observed
        .verified
        .expect("independent ordered chunk/whole digest and exact EOF check");
    assert!(observed.pattern_matches && observed.same_visible_inode);
    assert_eq!(observed.length, manifest.byte_length());
    assert_eq!(observed.observed_length, manifest.byte_length());
    assert_eq!(observed.digest, manifest.sha256().to_string());
    assert_eq!(observed.owner, root_identity(&f.target_root).2);
    assert_eq!(observed.mode & 0o7777, 0o400);
    assert_eq!(observed.links, 1);
    observed.identity
}
pub async fn denied(f: &Fixture, assembled: &AssembledFile, expected: FileSaveError) {
    let source = bytes::tree(&f.source_root);
    let destination = bytes::tree(&f.target_root);
    let source_root = bytes::snapshot(&f.source_root);
    let target_root = bytes::snapshot(&f.target_root);
    let result = tokio::time::timeout(
        Duration::from_secs(20),
        f.store
            .save_assembled_file(&f.manifest, assembled, &f.target),
    )
    .await;
    if result.is_err() {
        // Normal FIFO coverage has no writer. Rescue only a regressed blocking open so its
        // real worker can reach the regular-file rejection and drain before failure cleanup.
        let mut rescue = Vec::new();
        for path in [f.source_path(), f.target_path()] {
            if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_fifo()) {
                rescue.push(
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .custom_flags(libc::O_NONBLOCK)
                        .open(path)
                        .unwrap(),
                );
            }
        }
        if !rescue.is_empty() {
            restored(f).await;
        }
        drop(rescue);
    }
    let error = result
        .expect("unsafe output checks must finish without a blocking special-file open")
        .err()
        .expect("unsafe save must fail");
    assert_eq!(error, expected);
    for value in [
        f.source_root.to_string_lossy(),
        f.target_root.to_string_lossy(),
    ] {
        assert!(
            !error.to_string().contains(value.as_ref()),
            "errors contain no host pathname"
        );
    }
    assert_eq!(bytes::tree(&f.source_root), source);
    assert_eq!(bytes::tree(&f.target_root), destination);
    assert_eq!(bytes::snapshot(&f.source_root), source_root);
    assert_eq!(bytes::snapshot(&f.target_root), target_root);
}
pub fn changed(
    manifest: &FileTransferManifest,
    field: &str,
    value: serde_json::Value,
) -> FileTransferManifest {
    let mut raw = serde_json::to_value(manifest).unwrap();
    raw[field] = value;
    FileTransferManifest::parse_json(&serde_json::to_vec(&raw).unwrap()).unwrap()
}
pub fn private_directory(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}
pub fn mode(path: &Path, value: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(value)).unwrap();
}
pub fn write_private(path: &Path, data: &[u8]) {
    fs::write(path, data).unwrap();
    mode(path, 0o400);
}
pub async fn restored(f: &Fixture) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match f.store.verify_staged_chunk(&f.manifest, 0).await {
                Ok(()) => break,
                Err(ChunkStagingError::CapacityExhausted) => tokio::task::yield_now().await,
                Err(error) => panic!(
                    "original valid chunk must verify after this fixture job drains: {error}"
                ),
            }
        }
    })
    .await
    .expect("bounded real worker recovery");
}
