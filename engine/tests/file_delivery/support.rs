use crate::{fixture::*, *};
use std::{future::Future, path::Path, pin::Pin, time::Duration};

pub async fn within<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(30), future)
        .await
        .expect("this delivery operation must finish within its bounded real IO lifetime")
}

pub async fn deliver(
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

pub async fn metadata(
    f: &Fixture,
    client: &FileChunkClient,
) -> Result<FileTransferManifest, FileManifestClientError> {
    within(client.fetch_manifest(
        TOKEN,
        grant(),
        upload(),
        &FileVersionRef::from_manifest(&f.manifest),
    ))
    .await
}

pub fn metadata_error(error: FileManifestClientError) -> FileGrantedSaveError {
    FileGrantedSaveError::Download(FileGrantedDownloadError::Manifest(error))
}
pub fn chunk_error(index: u32, error: FileChunkClientError) -> FileGrantedSaveError {
    FileGrantedSaveError::Download(FileGrantedDownloadError::Download(
        FileDownloadError::Chunk { index, error },
    ))
}
pub fn staging_error(index: u32, error: ChunkStagingError) -> FileGrantedSaveError {
    chunk_error(
        index,
        FileChunkClientError::Receive(FileChunkReceiveError::Staging(error)),
    )
}
pub fn assembly_error(error: ChunkStagingError) -> FileGrantedSaveError {
    FileGrantedSaveError::Download(FileGrantedDownloadError::Download(
        FileDownloadError::Assembly(error),
    ))
}
pub fn changed(
    manifest: &FileTransferManifest,
    field: &str,
    value: serde_json::Value,
) -> FileTransferManifest {
    let mut json = serde_json::to_value(manifest).unwrap();
    json[field] = value;
    serde_json::from_value(json).unwrap()
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
pub fn assert_output(f: &Fixture, path: &Path) {
    let output = observe_output(path, &f.manifest).unwrap();
    output
        .verified
        .expect("independent ordered chunk SHA, whole SHA and EOF");
    assert!(output.pattern_matches && output.same_visible_inode);
    assert_eq!(output.mode & 0o7777, 0o400);
    assert_eq!(output.owner, root_identity(&f.root).2);
    assert_eq!(output.links, 1);
    assert_eq!(output.length, f.manifest.byte_length());
    assert_eq!(output.observed_length, f.manifest.byte_length());
    assert_eq!(output.digest, f.manifest.sha256().to_string());
}
pub fn redacted(f: &Fixture, error: &FileGrantedSaveError) {
    let text = format!("{error} {error:?}");
    for forbidden in [
        TOKEN.to_owned(),
        f.origin.clone(),
        grant().to_string(),
        upload().to_string(),
        f.root.to_string_lossy().into_owned(),
    ] {
        assert!(
            !text.contains(&forbidden),
            "static errors must not reflect credentials or paths"
        );
    }
}
pub async fn drive<F: Future + ?Sized>(f: &Fixture, count: usize, future: &mut Pin<Box<F>>) {
    tokio::select! {
        _ = future.as_mut() => panic!("held real response must not complete before its event"),
        _ = f.wait_requests(count) => {}
    }
}
