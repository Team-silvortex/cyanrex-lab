use crate::{fixture::*, *};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::Path,
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
pub fn assert_output(f: &Fixture, path: &Path) -> (u64, u64) {
    let value = observe(path, &f.manifest).unwrap();
    value
        .verified
        .expect("independent chunk SHA, whole SHA and exact EOF");
    assert!(value.same_visible_inode && value.pattern_matches);
    assert_eq!(value.mode & 0o7777, 0o400);
    assert_eq!(value.owner, directory_policy(&f.source_root).2);
    assert_eq!(value.links, 1);
    assert_eq!(value.length, f.manifest.byte_length());
    assert_eq!(value.observed_length, f.manifest.byte_length());
    assert_eq!(value.digest, f.manifest.sha256().to_string());
    value.identity
}
pub fn mode(path: &Path, bits: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(bits)).unwrap();
}
pub fn directory(path: &Path, bits: u32) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
    mode(path, bits);
}
pub fn write_private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    mode(path, 0o400);
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
pub async fn restored(f: &Fixture) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            match f.store.verify_staged_chunk(&f.manifest, 0).await {
                Ok(()) => break,
                Err(ChunkStagingError::CapacityExhausted) => tokio::task::yield_now().await,
                Err(error) => panic!("this fixture's actual worker must drain: {error}"),
            }
        }
    })
    .await
    .expect("bounded actual private worker recovery");
}
