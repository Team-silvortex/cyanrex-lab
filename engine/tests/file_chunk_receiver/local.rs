use crate::{fixture::*, *};
use axum::{body::Bytes, http::Response};
use futures_util::FutureExt;
use std::{
    fs, future::Future, os::unix::fs::DirBuilderExt, panic::AssertUnwindSafe, path::PathBuf,
    time::Duration,
};

pub struct Local {
    pub directory: PathBuf,
    pub root: PathBuf,
    pub staging: ChunkStaging,
}
impl Local {
    pub fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("cyanrex-receiver-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let root = directory.join("chunks");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let staging = ChunkStaging::open(scope(), &root, 1).unwrap();
        Self {
            directory,
            root,
            staging,
        }
    }
    pub fn receiver(&self, capacity: usize) -> FileChunkReceiver {
        FileChunkReceiver::with_limits(
            self.staging.clone(),
            capacity,
            Duration::from_secs(1),
            Duration::from_secs(3),
        )
        .unwrap()
    }
    pub fn tree(&self) -> Vec<bytes::Entry> {
        bytes::tree(&self.root)
    }
}
impl Drop for Local {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).expect("remove only the exclusive receiver fixture");
    }
}
pub async fn checked<F, Fut>(work: F)
where
    F: FnOnce(Local) -> Fut,
    Fut: Future<Output = ()>,
{
    // Local's exclusive RAII directory also cleans up when a behavior assertion fails.
    work(Local::new()).await;
}
pub async fn checked_sql<F>(mut f: Fixture, work: F)
where
    F: for<'a> FnOnce(&'a mut Fixture) -> std::pin::Pin<Box<dyn Future<Output = ()> + 'a>>,
{
    let result = AssertUnwindSafe(work(&mut f)).catch_unwind().await;
    f.cleanup().await;
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
pub fn values(manifest: &FileTransferManifest, index: u32) -> Vec<(&'static str, String)> {
    let chunk = manifest.chunk(index).unwrap();
    vec![
        ("content-type", "application/octet-stream".into()),
        ("content-length", chunk.byte_length.to_string()),
        (
            "content-disposition",
            "attachment; filename=\"chunk.bin\"".into(),
        ),
        ("x-cyanrex-file-transfer-version", "1".into()),
        ("x-cyanrex-file-id", manifest.file_id().to_string()),
        (
            "x-cyanrex-file-version-id",
            manifest.version_id().to_string(),
        ),
        ("x-cyanrex-chunk-index", index.to_string()),
        ("x-cyanrex-chunk-sha256", chunk.sha256.to_string()),
        ("x-cyanrex-file-sha256", manifest.sha256().to_string()),
        ("cache-control", "no-store".into()),
        ("pragma", "no-cache".into()),
        ("x-content-type-options", "nosniff".into()),
    ]
}
pub fn response<B>(manifest: &FileTransferManifest, index: u32, body: B) -> Response<B> {
    let mut response = Response::builder().status(200);
    for (name, value) in values(manifest, index) {
        response = response.header(name, value);
    }
    response.body(body).unwrap()
}
pub fn data(manifest: &FileTransferManifest, index: u32) -> Bytes {
    let chunk = manifest.chunk(index).unwrap();
    let start = index as u64 * FILE_TRANSFER_CHUNK_BYTES;
    (start..start + chunk.byte_length)
        .map(bytes::byte)
        .collect::<Vec<_>>()
        .into()
}
pub async fn rejected<B>(
    f: &Local,
    receiver: &FileChunkReceiver,
    manifest: &FileTransferManifest,
    index: u32,
    response: Response<B>,
    expected: FileChunkReceiveError,
) where
    B: http_body::Body<Data = Bytes> + Send + 'static,
{
    let before = f.tree();
    assert_eq!(
        receiver
            .receive_and_stage(manifest, index, response)
            .await
            .err(),
        Some(expected)
    );
    assert_eq!(
        f.tree(),
        before,
        "rejected reception must not create or alter a client entry"
    );
}
pub async fn healthy(f: &Local, receiver: &FileChunkReceiver, seed: u128) {
    let manifest = bytes::manifest(17, seed);
    let received = receiver
        .receive_and_stage(
            &manifest,
            0,
            response(&manifest, 0, Body::from(data(&manifest, 0))),
        )
        .await
        .unwrap();
    assert_eq!(received.file(), &FileVersionRef::from_manifest(&manifest));
    assert_eq!(received.chunk_index(), 0);
    assert_eq!(received.byte_length(), 17);
    assert_eq!(received.chunk_sha256(), &manifest.chunk(0).unwrap().sha256);
    let observed = observe_chunk(&f.root.join(legacy_name(&manifest, 0)), &manifest, 0).unwrap();
    assert_eq!(observed.mode & 0o777, 0o400);
    assert_eq!(observed.links, 1);
    assert_eq!(observed.length, 17);
    observed.verified.unwrap();
    assert_eq!(observed.bytes, data(&manifest, 0));
}
