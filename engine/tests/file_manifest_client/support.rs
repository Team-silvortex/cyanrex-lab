use crate::{fixture::*, *};
use std::{fmt, future::Future, pin::Pin, time::Duration};

pub fn config(capacity: usize) -> FileChunkClientConfig {
    FileChunkClientConfig {
        transport: FileChunkTransport::LoopbackHttp,
        capacity,
        header_timeout: Duration::from_secs(1),
        idle_timeout: Duration::from_secs(1),
        total_timeout: Duration::from_secs(3),
        trusted_root_pem: None,
    }
}
pub async fn within<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(20), future)
        .await
        .expect("bounded actual metadata/chunk operation must finish")
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
pub async fn combined(
    f: &Fixture,
    client: &FileChunkClient,
) -> Result<cyanrex_engine::services::file_transfer::staging::AssembledFile, FileGrantedDownloadError>
{
    within(client.fetch_granted_and_assemble(
        TOKEN,
        grant(),
        upload(),
        &FileVersionRef::from_manifest(&f.manifest),
    ))
    .await
}
pub async fn denied(f: &Fixture, client: &FileChunkClient, expected: FileManifestClientError) {
    let before = bytes::tree(&f.root);
    let error = metadata(f, client)
        .await
        .err()
        .expect("controlled bad metadata must fail");
    assert_eq!(error, expected);
    redacted(f, &error);
    assert_eq!(
        bytes::tree(&f.root),
        before,
        "metadata failures never dispatch file work"
    );
}
pub async fn healthy(f: &Fixture, client: &FileChunkClient) {
    f.clear_reply(Kind::Manifest);
    let before = bytes::tree(&f.root);
    assert_eq!(metadata(f, client).await.unwrap(), f.manifest);
    assert_eq!(bytes::tree(&f.root), before);
}
pub fn redacted(f: &Fixture, error: &impl fmt::Display) {
    let message = error.to_string();
    for secret in [
        TOKEN.to_owned(),
        f.origin.clone(),
        grant().to_string(),
        upload().to_string(),
    ] {
        assert!(
            !message.contains(&secret),
            "errors remain static and secret-free"
        );
    }
}
pub fn changed(
    manifest: &FileTransferManifest,
    field: &str,
    value: serde_json::Value,
) -> FileTransferManifest {
    let mut encoded = serde_json::to_value(manifest).unwrap();
    encoded[field] = value;
    serde_json::from_value(encoded).unwrap()
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
pub async fn drive_to_request<T, F: Future<Output = T> + ?Sized>(
    f: &Fixture,
    count: usize,
    pending: &mut Pin<Box<F>>,
) {
    tokio::select! {
        _ = pending.as_mut() => panic!("held real request must stay pending"),
        _ = f.wait_requests(count) => {}
    }
}
pub fn staging_error(index: u32, error: ChunkStagingError) -> FileGrantedDownloadError {
    FileGrantedDownloadError::Download(FileDownloadError::Chunk {
        index,
        error: FileChunkClientError::Receive(FileChunkReceiveError::Staging(error)),
    })
}
