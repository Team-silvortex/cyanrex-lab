use crate::{fixture::*, *};
use std::{future::Future, pin::Pin};

pub async fn drive_to_request<F: Future>(f: &Fixture, count: usize, pending: &mut Pin<Box<F>>) {
    tokio::select! {
        _ = pending.as_mut() => panic!("the selected actual HTTP phase must remain held"),
        _ = f.wait_requests(count) => {}
    }
}
pub fn chunk_error(index: u32, error: FileChunkClientError) -> FileDownloadError {
    FileDownloadError::Chunk { index, error }
}
pub fn staging_error(index: u32, error: ChunkStagingError) -> FileDownloadError {
    chunk_error(
        index,
        FileChunkClientError::Receive(FileChunkReceiveError::Staging(error)),
    )
}
pub fn invalid_response(index: u32) -> FileDownloadError {
    chunk_error(
        index,
        FileChunkClientError::Receive(FileChunkReceiveError::InvalidResponse),
    )
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
pub fn request_indices(requests: &[ObservedRequest]) -> Vec<u32> {
    requests
        .iter()
        .map(|request| request.query.as_ref().unwrap().chunk_index)
        .collect()
}
pub fn first_preserved(f: &Fixture, requests: &[ObservedRequest]) {
    assert_eq!(request_indices(requests), [0, 1]);
    assert!(requests[0].staged_before_request.is_empty());
    assert_eq!(requests[1].staged_before_request.len(), 1);
    assert_eq!(
        bytes::tree(&f.root),
        requests[1].staged_before_request,
        "failed later chunk must retain the original confirmed first chunk only"
    );
    assert!(!f.root.join(output_name(&f.manifest)).exists());
}
pub fn redacted(f: &Fixture, error: &FileDownloadError) {
    for text in [error.to_string(), format!("{error:?}")] {
        for secret in [
            TOKEN.to_owned(),
            f.origin.clone(),
            grant().to_string(),
            upload().to_string(),
        ] {
            assert!(
                !text.contains(&secret),
                "download errors must not expose private request values"
            );
        }
    }
}
