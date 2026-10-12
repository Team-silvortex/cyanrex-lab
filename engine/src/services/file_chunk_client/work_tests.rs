use super::*;
use axum::{
    body::{Body, Bytes},
    http::Response,
    routing::get,
    Router,
};
use http_body::Frame;
use sha2::{Digest, Sha256};
use std::{
    fs,
    future::{poll_fn, Future},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Condvar, Mutex,
    },
    task::{Context, Poll},
};
use tokio::{runtime::Builder, sync::oneshot, time::timeout};
use uuid::Uuid;

/// cfg(test)-only observation forwards every original frame/error unchanged, including None.
/// The unit also observes the enclosing fetch poll returning Pending, not this flag alone.
pub(super) struct ObservedBody {
    inner: reqwest::Body,
    eof: Option<Arc<AtomicBool>>,
}
impl ObservedBody {
    pub(super) fn new(inner: reqwest::Body, eof: Option<Arc<AtomicBool>>) -> Self {
        Self { inner, eof }
    }
}
impl http_body::Body for ObservedBody {
    type Data = Bytes;
    type Error = reqwest::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let frame = http_body::Body::poll_frame(Pin::new(&mut self.inner), cx);
        if matches!(&frame, Poll::Ready(None)) {
            if let Some(eof) = &self.eof {
                eof.store(true, Ordering::SeqCst);
            }
        }
        frame
    }
}

struct Root(PathBuf, (u64, u64));
impl Root {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cyanrex-chunk-client-unit-{}", Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        let meta = fs::metadata(&path).unwrap();
        Self(path, (meta.dev(), meta.ino()))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove only this exclusive synthetic client root");
    }
}
struct Barrier {
    released: Mutex<bool>,
    wake: Condvar,
}
impl Barrier {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            released: Mutex::new(false),
            wake: Condvar::new(),
        })
    }
    fn wait(&self) {
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.wake.wait(released).unwrap();
        }
    }
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.wake.notify_all();
    }
}
struct ReleaseOnDrop(Arc<Barrier>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}
struct Stop(Option<oneshot::Sender<()>>);
impl Drop for Stop {
    fn drop(&mut self) {
        if let Some(stop) = self.0.take() {
            let _ = stop.send(());
        }
    }
}

fn data() -> Vec<u8> {
    (0..17).collect()
}
fn manifest() -> FileTransferManifest {
    let workspace = WorkspaceRef {
        authority_id: Uuid::from_u128(1).try_into().unwrap(),
        workspace_id: Uuid::from_u128(2).try_into().unwrap(),
    };
    let digest: crate::models::collaboration::Sha256Digest =
        format!("{:x}", Sha256::digest(data())).parse().unwrap();
    FileTransferManifest::new(
        workspace,
        Uuid::from_u128(3).try_into().unwrap(),
        Uuid::from_u128(4).try_into().unwrap(),
        "client.bin",
        17,
        vec![digest.clone()],
        digest,
    )
    .unwrap()
}
fn response(manifest: &FileTransferManifest) -> Response<Body> {
    let spec = manifest.chunk(0).unwrap();
    let mut builder = Response::builder().status(200);
    for (name, value) in [
        ("content-type", "application/octet-stream".to_owned()),
        (
            "content-disposition",
            "attachment; filename=\"chunk.bin\"".to_owned(),
        ),
        ("content-length", "17".to_owned()),
        ("x-cyanrex-file-transfer-version", "1".to_owned()),
        ("x-cyanrex-file-id", manifest.file_id().to_string()),
        (
            "x-cyanrex-file-version-id",
            manifest.version_id().to_string(),
        ),
        ("x-cyanrex-chunk-index", "0".to_owned()),
        ("x-cyanrex-chunk-sha256", spec.sha256.to_string()),
        ("x-cyanrex-file-sha256", manifest.sha256().to_string()),
        ("cache-control", "no-store".to_owned()),
        ("pragma", "no-cache".to_owned()),
        ("x-content-type-options", "nosniff".to_owned()),
    ] {
        builder = builder.header(name, value);
    }
    builder.body(Body::from(data())).unwrap()
}

#[test]
fn cancelled_client_after_real_http_eof_retains_flow_capacity_until_actual_queued_save_finishes() {
    let root = Root::new();
    let runtime = Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let barrier = Barrier::new();
    let _release = ReleaseOnDrop(barrier.clone());
    let (entered, started) = mpsc::channel();
    let held = barrier.clone();
    let external = runtime.spawn_blocking(move || {
        entered.send(()).unwrap();
        held.wait();
    });
    started
        .recv_timeout(Duration::from_secs(10))
        .expect("external job really holds the only blocking thread");
    runtime.block_on(async {
        let manifest = manifest();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let served = manifest.clone();
        let router = Router::new().route("/platform/v1/file-chunks", get(move || {
            let counter = counter.clone(); let served = served.clone();
            async move { counter.fetch_add(1, Ordering::SeqCst); response(&served) }
        }));
        let (stop, stopped) = oneshot::channel();
        let stop = Stop(Some(stop));
        let server = runtime.spawn(async move {
            axum::serve(listener, router).with_graceful_shutdown(async { let _ = stopped.await; }).await.unwrap();
        });
        let staging = ChunkStaging::open(manifest.workspace(), &root.0, 1).unwrap();
        let mut client = FileChunkClient::with_config(&origin, staging.clone(), FileChunkClientConfig {
            transport: FileChunkTransport::LoopbackHttp, capacity: 1, ..FileChunkClientConfig::default()
        }).unwrap();
        let eof = Arc::new(AtomicBool::new(false));
        Arc::get_mut(&mut client.inner).unwrap().eof_observer = Some(eof.clone());
        let token = "ed632649-aa61-4b26-a28f-e592803449d8";
        let grant = Uuid::from_u128(5).try_into().unwrap();
        let upload = Uuid::from_u128(6).try_into().unwrap();
        let mut pending = Box::pin(client.fetch_and_stage(token, grant, upload, &manifest, 0));
        timeout(Duration::from_secs(10), poll_fn(|cx| {
            match pending.as_mut().poll(cx) {
                Poll::Pending if eof.load(Ordering::SeqCst) => Poll::Ready(()),
                Poll::Pending => Poll::Pending,
                Poll::Ready(_) => panic!("complete fetch poll after real EOF must wait on queued save"),
            }
        })).await.unwrap();
        // No await exists after the collector's actual None/digest and before stage's JoinHandle:
        // this complete fetch Pending plus the held blocking thread proves real queued dispatch.
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(staging.verify_staged_chunk(&manifest, 0).await.err(),
            Some(crate::services::file_transfer::staging::ChunkStagingError::CapacityExhausted));
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
        drop(pending);
        assert_eq!(client.fetch_and_stage(token, grant, upload, &manifest, 0).await.err(),
            Some(FileChunkClientError::CapacityExhausted));
        assert_eq!(hits.load(Ordering::SeqCst), 1, "no new request admitted while orphan save owns caller lease");
        assert_eq!(staging.verify_staged_chunk(&manifest, 0).await.err(),
            Some(crate::services::file_transfer::staging::ChunkStagingError::CapacityExhausted));
        barrier.release();
        timeout(Duration::from_secs(10), external).await.unwrap().unwrap();
        let slot = timeout(Duration::from_secs(10), client.inner.controls.clone().acquire_owned()).await.unwrap().unwrap();
        drop(slot);
        timeout(Duration::from_secs(10), async {
            loop {
                match staging.verify_staged_chunk(&manifest, 0).await {
                    Ok(()) => break,
                    Err(crate::services::file_transfer::staging::ChunkStagingError::CapacityExhausted) => tokio::task::yield_now().await,
                    Err(error) => panic!("real cancelled save output must remain verified: {error}"),
                }
            }
        }).await.unwrap();
        let files = fs::read_dir(&root.0).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(files.len(), 1);
        let path = files[0].path();
        let before = fs::metadata(&path).unwrap();
        assert_eq!(before.mode() & 0o7777, 0o400);
        assert_eq!(before.nlink(), 1);
        assert_eq!(fs::read(&path).unwrap(), data());
        assert_eq!(client.fetch_and_stage(token, grant, upload, &manifest, 0).await.err(),
            Some(FileChunkClientError::Receive(FileChunkReceiveError::Staging(
                crate::services::file_transfer::staging::ChunkStagingError::Conflict))));
        let after = fs::metadata(&path).unwrap();
        assert_eq!((after.dev(), after.ino(), after.len(), after.mtime(), after.mtime_nsec(), after.ctime(), after.ctime_nsec()),
            (before.dev(), before.ino(), before.len(), before.mtime(), before.mtime_nsec(), before.ctime(), before.ctime_nsec()));
        let root_meta = fs::metadata(&root.0).unwrap();
        assert_eq!((root_meta.dev(), root_meta.ino()), root.1);
        assert_eq!(root_meta.mode() & 0o7777, 0o700);
        let fresh = Root::new();
        let fresh_client = FileChunkClient::new_loopback(&origin, ChunkStaging::open(manifest.workspace(), &fresh.0, 1).unwrap()).unwrap();
        let ack = fresh_client.fetch_and_stage(token, grant, upload, &manifest, 0).await.unwrap();
        assert_eq!(ack.byte_length(), 17);
        assert_eq!(ack.file(), &crate::models::collaboration::FileVersionRef::from_manifest(&manifest));
        drop(stop);
        timeout(Duration::from_secs(10), server).await.unwrap().unwrap();
    });
}
