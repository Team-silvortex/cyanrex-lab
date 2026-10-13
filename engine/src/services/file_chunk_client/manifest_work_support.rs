//! cfg(test)-only actual loopback fixture; no production caller can provide a body or observer.
use super::{download_test_support::*, *};
use axum::{
    body::{Body, Bytes},
    extract::Query,
    http::Response,
    routing::get,
    Router,
};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};
use tokio::{
    sync::{oneshot, Notify, Semaphore},
    task::JoinHandle,
    time::timeout,
};

pub(super) struct Server {
    pub(super) origin: String,
    pub(super) metadata: Arc<AtomicUsize>,
    pub(super) chunks: Arc<Mutex<Vec<u32>>>,
    pub(super) entered: Arc<Notify>,
    pub(super) release: Arc<Semaphore>,
    stop: Option<oneshot::Sender<()>>,
    job: Option<JoinHandle<()>>,
}
impl Server {
    pub(super) async fn start(
        manifest: &FileTransferManifest,
        bytes: Vec<u8>,
        hold_first: bool,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let metadata = Arc::new(AtomicUsize::new(0));
        let chunks = Arc::new(Mutex::new(Vec::new()));
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Semaphore::new(0));
        let encoded = serde_json::to_vec(manifest).unwrap();
        let count = metadata.clone();
        let observer = entered.clone();
        let gate = release.clone();
        let metadata_route = get(move || {
            let count = count.clone();
            let observer = observer.clone();
            let gate = gate.clone();
            let encoded = encoded.clone();
            async move {
                let first = count.fetch_add(1, Ordering::SeqCst);
                let hold = hold_first && first == 0;
                let length = encoded.len();
                let body = Body::from_stream(futures_util::stream::once(async move {
                    if hold {
                        observer.notify_one();
                        gate.acquire().await.unwrap().forget();
                    }
                    Ok::<_, std::io::Error>(Bytes::from(encoded))
                }));
                Response::builder()
                    .status(200)
                    .header("content-length", length.to_string())
                    .header("content-type", "application/json")
                    .header("cache-control", "no-store")
                    .header("pragma", "no-cache")
                    .header("x-content-type-options", "nosniff")
                    .body(body)
                    .unwrap()
            }
        });
        let recorded = chunks.clone();
        let manifest = manifest.clone();
        let bytes = Arc::new(bytes);
        let chunk_route = get(move |Query(query): Query<BTreeMap<String, String>>| {
            let recorded = recorded.clone();
            let manifest = manifest.clone();
            let bytes = bytes.clone();
            async move {
                let index = query.get("chunk_index").unwrap().parse::<u32>().unwrap();
                recorded.lock().unwrap().push(index);
                let spec = manifest.chunk(index).unwrap();
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
        });
        let router = Router::new()
            .route("/platform/v1/file-manifests", metadata_route)
            .route("/platform/v1/file-chunks", chunk_route);
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
            metadata,
            chunks,
            entered,
            release,
            stop: Some(stop),
            job: Some(job),
        }
    }
    pub(super) async fn finish(mut self) {
        self.release.add_permits(1);
        let _ = self.stop.take().unwrap().send(());
        timeout(Duration::from_secs(10), self.job.as_mut().unwrap())
            .await
            .unwrap()
            .unwrap();
        self.job.take();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.release.add_permits(1);
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(job) = &self.job {
            job.abort();
        }
    }
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
        .fetch_granted_and_assemble(
            TOKEN,
            grant,
            upload,
            &FileVersionRef::from_manifest(manifest),
        )
        .await
        .unwrap();
    assert_eq!(ack.file(), &FileVersionRef::from_manifest(manifest));
    verify_output(root, manifest, bytes);
    assert_eq!(client.inner.downloads.available_permits(), 1);
    assert_eq!(client.inner.manifests.available_permits(), 1);
}
