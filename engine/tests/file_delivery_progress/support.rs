use crate::{fixture::*, *};
use std::{fs, future::Future, path::Path, pin::Pin, time::Duration};

pub async fn within<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(30), future)
        .await
        .expect("bounded actual observed delivery/transport/file operation")
}
pub async fn observed(
    f: &Fixture,
    client: &FileChunkClient,
    target: &FileSaveTarget,
    observer: FileDeliveryObserver,
) -> Result<FileVersionRef, FileGrantedSaveError> {
    within(client.fetch_granted_and_save_observed(
        TOKEN,
        grant(),
        upload(),
        &FileVersionRef::from_manifest(&f.manifest),
        target,
        observer,
    ))
    .await
    .map(|saved| saved.file().clone())
}
pub async fn plain(
    f: &Fixture,
    client: &FileChunkClient,
    target: &FileSaveTarget,
) -> Result<FileVersionRef, FileGrantedSaveError> {
    within(client.fetch_granted_and_save(
        TOKEN,
        grant(),
        upload(),
        &FileVersionRef::from_manifest(&f.manifest),
        target,
    ))
    .await
    .map(|saved| saved.file().clone())
}
pub fn snapshot(
    f: &Fixture,
    progress: &FileDeliveryProgress,
    phase: FileDeliveryPhase,
    chunks: u32,
    bytes: u64,
    total_chunks: Option<u32>,
) -> FileDeliverySnapshot {
    let value = progress
        .latest()
        .expect("this real current-call boundary has a snapshot");
    assert_eq!(value.phase(), phase);
    assert_eq!(value.staged_chunks(), chunks);
    assert_eq!(value.staged_bytes(), bytes);
    assert_eq!(value.total_bytes(), f.manifest.byte_length());
    assert_eq!(value.total_chunks(), total_chunks);
    let text = format!("{value:?}");
    for hidden in [
        TOKEN.to_owned(),
        f.origin.clone(),
        grant().to_string(),
        upload().to_string(),
        f.manifest.file_id().to_string(),
        f.manifest.version_id().to_string(),
        f.manifest.sha256().to_string(),
        f.manifest.display_name().to_owned(),
        f.root.to_string_lossy().into_owned(),
    ] {
        assert!(
            !text.contains(&hidden),
            "local scalar progress reflects no credentials, names or paths"
        );
    }
    value
}
pub async fn closed(progress: &mut FileDeliveryProgress, expected: Option<FileDeliverySnapshot>) {
    // A closed watch can still deliver its one last unseen value. Latest does not consume it.
    for _ in 0..2 {
        let next = tokio::time::timeout(Duration::from_secs(3), progress.changed())
            .await
            .expect("producer termination must end observation, not retain a job sender");
        match next {
            Some(value) => assert_eq!(Some(value), expected),
            None => {
                assert_eq!(progress.latest(), expected);
                return;
            }
        }
    }
    panic!("closed producer has one latest slot, never an event queue");
}
pub async fn drive<F: Future + ?Sized>(f: &Fixture, count: usize, future: &mut Pin<Box<F>>) {
    tokio::select! {
        _ = future.as_mut() => panic!("held real response must not complete before its request event"),
        _ = f.wait_requests(count) => {}
    }
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
pub fn indices(requests: &[ObservedRequest]) -> Vec<Option<u32>> {
    requests
        .iter()
        .map(|request| match request.selected.as_ref().unwrap() {
            Selected::Manifest(_) => None,
            Selected::Chunk(query) => Some(query.chunk_index),
        })
        .collect()
}
pub fn chunk_error(index: u32, error: FileChunkClientError) -> FileGrantedSaveError {
    FileGrantedSaveError::Download(FileGrantedDownloadError::Download(
        FileDownloadError::Chunk { index, error },
    ))
}
pub fn staging_conflict() -> FileGrantedSaveError {
    chunk_error(
        0,
        FileChunkClientError::Receive(FileChunkReceiveError::Staging(ChunkStagingError::Conflict)),
    )
}
pub fn assert_output(f: &Fixture, path: &Path) {
    let value = observe_output(path, &f.manifest).unwrap();
    value
        .verified
        .expect("independent chunk SHA, whole SHA and EOF");
    assert!(value.pattern_matches && value.same_visible_inode);
    assert_eq!(value.mode & 0o7777, 0o400);
    assert_eq!(value.owner, directory_policy(&f.root).2);
    assert_eq!(value.links, 1);
    assert_eq!(value.length, f.manifest.byte_length());
    assert_eq!(value.observed_length, f.manifest.byte_length());
    assert_eq!(value.digest, f.manifest.sha256().to_string());
}
pub fn verify_prefix(f: &Fixture, index: u32) {
    verify_chunk(
        &f.manifest,
        index,
        &mut fs::File::open(f.root.join(chunk_name(&f.manifest, index))).unwrap(),
    )
    .unwrap();
}
