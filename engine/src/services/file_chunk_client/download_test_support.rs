use super::*;
use crate::{
    models::collaboration::FileVersionRef,
    services::file_transfer::{inspect_stream, verify_file},
};
use axum::{body::Body, extract::Query, http::Response, routing::get, Router};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Cursor,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::PathBuf,
    sync::{Condvar, Mutex},
};
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};
use uuid::Uuid;

pub(super) const TOKEN: &str = "ed632649-aa61-4b26-a28f-e592803449d8";
pub(super) struct Root(pub(super) PathBuf, (u64, u64, u32));
impl Root {
    pub(super) fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-download-private-{}", Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        let meta = fs::metadata(&path).unwrap();
        Self(path, (meta.dev(), meta.ino(), meta.mode() & 0o7777))
    }
    pub(super) fn unchanged(&self) {
        let meta = fs::metadata(&self.0).unwrap();
        assert_eq!((meta.dev(), meta.ino(), meta.mode() & 0o7777), self.1);
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) struct Barrier {
    released: Mutex<bool>,
    wake: Condvar,
}
impl Barrier {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            released: Mutex::new(false),
            wake: Condvar::new(),
        })
    }
    pub(super) fn wait(&self) {
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.wake.wait(released).unwrap();
        }
    }
    pub(super) fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
    }
}
pub(super) struct ReleaseOnDrop(pub(super) Arc<Barrier>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}

pub(super) fn manifest(bytes: &[u8]) -> FileTransferManifest {
    inspect_stream(
        WorkspaceRef {
            authority_id: Uuid::from_u128(1).try_into().unwrap(),
            workspace_id: Uuid::from_u128(2).try_into().unwrap(),
        },
        Uuid::from_u128(3).try_into().unwrap(),
        Uuid::from_u128(4).try_into().unwrap(),
        "download.bin",
        &mut Cursor::new(bytes),
    )
    .unwrap()
}
pub(super) fn ids() -> (FileGrantId, FileUploadId) {
    (
        Uuid::from_u128(5).try_into().unwrap(),
        Uuid::from_u128(6).try_into().unwrap(),
    )
}
pub(super) fn client(
    origin: &str,
    root: &Root,
    manifest: &FileTransferManifest,
) -> FileChunkClient {
    FileChunkClient::with_config(
        origin,
        ChunkStaging::open(manifest.workspace(), &root.0, 1).unwrap(),
        FileChunkClientConfig {
            transport: FileChunkTransport::LoopbackHttp,
            capacity: 1,
            ..FileChunkClientConfig::default()
        },
    )
    .unwrap()
}
pub(super) fn name(manifest: &FileTransferManifest, prefix: &str, index: Option<u32>) -> String {
    let digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    );
    match index {
        Some(index) => format!("{prefix}-{digest}-{index}"),
        None => format!("{prefix}-{digest}"),
    }
}
pub(super) fn verify_output(root: &Root, manifest: &FileTransferManifest, bytes: &[u8]) {
    let path = root.0.join(name(manifest, "f1", None));
    let metadata = fs::metadata(&path).unwrap();
    assert_eq!(metadata.len(), bytes.len() as u64);
    assert_eq!(metadata.mode() & 0o7777, 0o400);
    assert_eq!(metadata.nlink(), 1);
    assert_eq!(metadata.uid(), fs::metadata(&root.0).unwrap().uid());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    verify_file(manifest, &mut fs::File::open(&path).unwrap()).unwrap();
    root.unchanged();
}
pub(super) async fn restored(client: &FileChunkClient) {
    let lease = timeout(
        Duration::from_secs(10),
        client.inner.downloads.clone().acquire_owned(),
    )
    .await
    .unwrap()
    .unwrap();
    drop(lease);
}
pub(super) async fn positive(
    origin: &str,
    root: &Root,
    manifest: &FileTransferManifest,
    bytes: &[u8],
) {
    let client = client(origin, root, manifest);
    let (grant, upload) = ids();
    let ack = client
        .fetch_and_assemble(TOKEN, grant, upload, manifest)
        .await
        .unwrap();
    assert_eq!(ack.file(), &FileVersionRef::from_manifest(manifest));
    verify_output(root, manifest, bytes);
    assert_eq!(client.inner.downloads.available_permits(), 1);
}

pub(super) struct Server {
    pub(super) origin: String,
    pub(super) hits: Arc<Mutex<Vec<u32>>>,
    stop: Option<oneshot::Sender<()>>,
    job: Option<JoinHandle<()>>,
}
impl Server {
    pub(super) async fn start(manifest: &FileTransferManifest, bytes: Vec<u8>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(Mutex::new(Vec::new()));
        let recorded = hits.clone();
        let manifest = manifest.clone();
        let bytes = Arc::new(bytes);
        let router = Router::new().route(
            "/platform/v1/file-chunks",
            get(move |Query(query): Query<BTreeMap<String, String>>| {
                let hits = recorded.clone();
                let manifest = manifest.clone();
                let bytes = bytes.clone();
                async move {
                    let index: u32 = query.get("chunk_index").unwrap().parse().unwrap();
                    hits.lock().unwrap().push(index);
                    let Ok(spec) = manifest.chunk(index) else {
                        return Response::builder().status(400).body(Body::empty()).unwrap();
                    };
                    let mut response = Response::builder().status(200);
                    for (key, value) in [
                        ("content-length", spec.byte_length.to_string()),
                        ("content-type", "application/octet-stream".into()),
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
                        ("x-cyanrex-chunk-sha256", spec.sha256.to_string()),
                        ("x-cyanrex-file-sha256", manifest.sha256().to_string()),
                        ("cache-control", "no-store".into()),
                        ("pragma", "no-cache".into()),
                        ("x-content-type-options", "nosniff".into()),
                    ] {
                        response = response.header(key, value);
                    }
                    response
                        .body(Body::from(
                            bytes[spec.offset as usize..(spec.offset + spec.byte_length) as usize]
                                .to_vec(),
                        ))
                        .unwrap()
                }
            }),
        );
        let (stop, stopped) = oneshot::channel();
        let job = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        Self {
            origin,
            hits,
            stop: Some(stop),
            job: Some(job),
        }
    }
    pub(super) async fn finish(mut self) {
        let _ = self.stop.take().unwrap().send(());
        timeout(Duration::from_secs(10), self.job.take().unwrap())
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
