// The controlled HTTP fixture stays read-only, with its own independent legacy HTTP scope.
#[allow(dead_code, unused_imports)]
#[path = "../file_manifest_client/fixture.rs"]
mod transport;
pub use crate::reply::{Kind, Piece, Reply};
use crate::*;
use std::{
    fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};
pub use transport::{
    assert_request, checked, grant, observe_output, output_name, upload, Fixture, ObservedRequest,
    Selected, TOKEN,
};

pub fn manifest(length: u64) -> FileTransferManifest {
    let payload = bytes::manifest(length, 1);
    // The root bytes fixture intentionally belongs to local save scope. Reuse only its independent
    // payload hashes/pattern; explicitly bind this manifest to the HTTP fixture's existing scope.
    FileTransferManifest::new(
        crate::legacy_fixture::scope(),
        payload.file_id(),
        payload.version_id(),
        payload.display_name(),
        length,
        payload.chunk_sha256().to_vec(),
        payload.sha256().clone(),
    )
    .unwrap()
}
pub fn selected_directory(f: &Fixture) -> PathBuf {
    let selected = f
        .root
        .parent()
        .unwrap()
        .join("ordinary-network-selected-parent");
    crate::support::directory(&selected, 0o755);
    selected
}
pub async fn deliver(
    f: &Fixture,
    client: &FileChunkClient,
    target: &FileSaveTarget,
) -> Result<FileVersionRef, FileGrantedSaveError> {
    let expected = FileVersionRef::from_manifest(&f.manifest);
    crate::fixture::within(client.fetch_granted_and_save(
        TOKEN,
        grant(),
        upload(),
        &expected,
        target,
    ))
    .await
    .map(|saved| saved.file().clone())
}
pub fn indices(requests: &[ObservedRequest]) -> Vec<Option<u32>> {
    requests
        .iter()
        .map(|request| match request.selected.as_ref().unwrap() {
            Selected::Manifest(_) => None,
            Selected::Chunk(query) => Some(query.chunk_index),
        })
        .collect()
}
pub fn assert_whole(f: &Fixture, path: &Path) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    let observed = observe_output(path, &f.manifest).unwrap();
    observed
        .verified
        .expect("independent ordered chunk/whole SHA and exact EOF");
    assert!(observed.same_visible_inode && observed.pattern_matches);
    assert_eq!(observed.mode & 0o7777, 0o400);
    assert_eq!(observed.owner, crate::fixture::directory_policy(&f.root).2);
    assert_eq!(observed.links, 1);
    assert_eq!(observed.length, f.manifest.byte_length());
    assert_eq!(observed.observed_length, f.manifest.byte_length());
    assert_eq!(observed.digest, f.manifest.sha256().to_string());
    let metadata = fs::metadata(path).unwrap();
    (metadata.dev(), metadata.ino())
}
pub async fn drive<F: Future + ?Sized>(f: &Fixture, count: usize, future: &mut Pin<Box<F>>) {
    tokio::select! {
        _ = future.as_mut() => panic!("held actual metadata response must not complete"),
        _ = f.wait_requests(count) => {}
    }
}
