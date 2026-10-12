use crate::{fixture::*, *};
use futures_util::FutureExt;
use std::{future::Future, panic::AssertUnwindSafe, pin::Pin, time::Duration};

pub async fn checked<F>(mut f: Fixture, work: F)
where
    F: for<'a> FnOnce(&'a mut Fixture) -> Pin<Box<dyn Future<Output = ()> + 'a>>,
{
    let outcome = AssertUnwindSafe(work(&mut f)).catch_unwind().await;
    f.cleanup().await;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
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
pub fn client(f: &Fixture, capacity: usize) -> FileChunkClient {
    FileChunkClient::with_config(&f.origin, f.staging.clone(), config(capacity)).unwrap()
}
pub fn metadata_error() -> FileChunkClientError {
    FileChunkClientError::Receive(
        cyanrex_engine::services::file_chunk_receiver::FileChunkReceiveError::InvalidResponse,
    )
}
pub fn timed_body() -> FileChunkClientError {
    FileChunkClientError::Receive(
        cyanrex_engine::services::file_chunk_receiver::FileChunkReceiveError::TimedOut,
    )
}
pub async fn denied(
    f: &Fixture,
    client: &FileChunkClient,
    manifest: &FileTransferManifest,
    index: u32,
    expected: FileChunkClientError,
) {
    let before = bytes::tree(&f.root);
    let result = client
        .fetch_and_stage(TOKEN, grant(), upload(), manifest, index)
        .await;
    assert_eq!(result.err(), Some(expected));
    assert_eq!(bytes::tree(&f.root), before);
}
pub async fn healthy(f: &Fixture, client: &FileChunkClient, seed: u128) {
    let manifest = bytes::manifest(17, seed);
    f.set_reply(Reply::good(&manifest, 0));
    let received = client
        .fetch_and_stage(TOKEN, grant(), upload(), &manifest, 0)
        .await
        .unwrap();
    assert_eq!(received.file(), &FileVersionRef::from_manifest(&manifest));
    assert_eq!(received.chunk_index(), 0);
    assert_eq!(received.byte_length(), 17);
    assert_eq!(received.chunk_sha256(), &manifest.chunk(0).unwrap().sha256);
    let observed = saved(&f.root.join(name(&manifest)), &manifest).unwrap();
    observed.verified.unwrap();
    assert_eq!(observed.mode & 0o777, 0o400);
    assert_eq!(observed.links, 1);
    assert_eq!(observed.bytes, (0..17).map(bytes::byte).collect::<Vec<_>>());
}
