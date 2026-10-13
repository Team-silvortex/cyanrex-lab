pub use crate::legacy_fixture::{
    chunk_name, grant, observe_output, output_name, scope, upload, TOKEN,
};
pub use crate::reply::{Kind, Piece, Reply};
use crate::*;
use axum::{
    body::Body,
    extract::{Query, State},
    http::{HeaderMap, Method, Request, Response, StatusCode, Uri},
    Router,
};
use futures_util::FutureExt;
use http_body_util::BodyExt;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    future::Future,
    io,
    os::unix::fs::DirBuilderExt,
    panic::AssertUnwindSafe,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManifestQuery {
    pub grant_id: FileGrantId,
    pub upload_id: FileUploadId,
    pub file_id: FileId,
    pub version_id: FileVersionId,
    pub byte_length: u64,
    pub sha256: Sha256Digest,
}
#[derive(Debug)]
pub enum Selected {
    Manifest(ManifestQuery),
    Chunk(crate::legacy_fixture::RequestQuery),
}
#[derive(Debug)]
pub struct ObservedRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
    pub selected: Result<Selected, String>,
    pub staged_before_request: Vec<bytes::Entry>,
}
#[derive(Clone)]
struct ServerState {
    manifest: FileTransferManifest,
    root: PathBuf,
    observed: Arc<Mutex<Vec<ObservedRequest>>>,
    replies: Arc<Mutex<BTreeMap<Kind, Reply>>>,
    produced: Arc<Mutex<BTreeMap<Kind, usize>>>,
}
async fn handle(State(state): State<ServerState>, request: Request<Body>) -> Response<Body> {
    let (parts, body) = request.into_parts();
    let selected = match parts.uri.path() {
        "/platform/v1/file-manifests" => Query::<ManifestQuery>::try_from_uri(&parts.uri)
            .map(|query| Selected::Manifest(query.0))
            .map_err(|error| error.to_string()),
        "/platform/v1/file-chunks" => {
            Query::<crate::legacy_fixture::RequestQuery>::try_from_uri(&parts.uri)
                .map(|query| Selected::Chunk(query.0))
                .map_err(|error| error.to_string())
        }
        _ => Err("unexpected controlled endpoint".to_owned()),
    };
    let body = body.collect().await.unwrap().to_bytes().to_vec();
    let staged_before_request = bytes::tree(&state.root);
    // Freeze each response before publishing its accepted-request event.
    let response = selected
        .as_ref()
        .ok()
        .and_then(|selected| match selected {
            Selected::Manifest(_) => Some(Kind::Manifest),
            Selected::Chunk(query) => state
                .manifest
                .chunk(query.chunk_index)
                .ok()
                .map(|_| Kind::Chunk(query.chunk_index)),
        })
        .map(|kind| {
            let reply = state
                .replies
                .lock()
                .unwrap()
                .get(&kind)
                .cloned()
                .unwrap_or_else(|| Reply::good(&state.manifest, kind));
            (kind, reply)
        });
    state.observed.lock().unwrap().push(ObservedRequest {
        method: parts.method,
        uri: parts.uri,
        headers: parts.headers,
        body,
        selected,
        staged_before_request,
    });
    if let Some((kind, reply)) = response {
        reply.response(kind, state.produced).await
    } else {
        Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::empty())
            .unwrap()
    }
}

struct DirectoryGuard(PathBuf);
impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            if error.kind() != io::ErrorKind::NotFound {
                eprintln!("exclusive manifest client fixture cleanup failed: {error}");
            }
        }
    }
}
struct ServerGuard {
    job: Option<tokio::task::JoinHandle<io::Result<()>>>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}
impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(server) = &self.job {
            server.abort();
        }
    }
}
pub struct Fixture {
    pub origin: String,
    pub root: PathBuf,
    pub staging: ChunkStaging,
    pub manifest: FileTransferManifest,
    observed: Arc<Mutex<Vec<ObservedRequest>>>,
    replies: Arc<Mutex<BTreeMap<Kind, Reply>>>,
    produced: Arc<Mutex<BTreeMap<Kind, usize>>>,
    gates: Mutex<Vec<Arc<tokio::sync::Semaphore>>>,
    server: ServerGuard,
    cleanup: DirectoryGuard,
}
impl Fixture {
    pub async fn new() -> Self {
        Self::with_manifest(bytes::manifest(FILE_TRANSFER_CHUNK_BYTES + 17, 1)).await
    }
    pub async fn with_manifest(manifest: FileTransferManifest) -> Self {
        let directory =
            std::env::temp_dir().join(format!("cyanrex-manifest-client-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let cleanup = DirectoryGuard(directory.clone());
        let root = directory.join("chunks");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let staging = ChunkStaging::open(scope(), &root, 1).unwrap();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let replies = Arc::new(Mutex::new(BTreeMap::new()));
        let produced = Arc::new(Mutex::new(BTreeMap::new()));
        let state = ServerState {
            manifest: manifest.clone(),
            root: root.clone(),
            observed: observed.clone(),
            replies: replies.clone(),
            produced: produced.clone(),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new().fallback(handle).with_state(state);
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let server = ServerGuard {
            job: Some(tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        let _ = stopped.await;
                    })
                    .await
            })),
            stop: Some(stop),
        };
        Self {
            origin,
            root,
            staging,
            manifest,
            observed,
            replies,
            produced,
            gates: Mutex::new(Vec::new()),
            server,
            cleanup,
        }
    }
    pub fn client(&self) -> FileChunkClient {
        self.client_with(FileChunkClientConfig {
            transport: FileChunkTransport::LoopbackHttp,
            capacity: 1,
            ..FileChunkClientConfig::default()
        })
    }
    pub fn client_with(&self, config: FileChunkClientConfig) -> FileChunkClient {
        FileChunkClient::with_config(&self.origin, self.staging.clone(), config).unwrap()
    }
    pub fn take_requests(&self) -> Vec<ObservedRequest> {
        std::mem::take(&mut *self.observed.lock().unwrap())
    }
    pub fn request_count(&self) -> usize {
        self.observed.lock().unwrap().len()
    }
    pub fn set_reply(&self, kind: Kind, reply: Reply) {
        let mut gates = self.gates.lock().unwrap();
        gates.extend(reply.header_gate.iter().cloned());
        gates.extend(reply.pieces.iter().filter_map(|piece| piece.gate.clone()));
        self.replies.lock().unwrap().insert(kind, reply);
    }
    pub fn clear_reply(&self, kind: Kind) {
        self.replies.lock().unwrap().remove(&kind);
    }
    pub fn produced(&self, kind: Kind) -> usize {
        self.produced
            .lock()
            .unwrap()
            .get(&kind)
            .copied()
            .unwrap_or(0)
    }
    pub async fn wait_requests(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.request_count() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("real loopback server must observe the request");
    }
    pub async fn wait_pieces(&self, kind: Kind, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.produced(kind) < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("real framed body must yield the selected prefix");
    }
    pub async fn cleanup(mut self) {
        for gate in self.gates.lock().unwrap().iter() {
            gate.add_permits(128);
        }
        let _ = self.server.stop.take().unwrap().send(());
        let joined =
            tokio::time::timeout(Duration::from_secs(5), self.server.job.as_mut().unwrap()).await;
        if joined.is_err() {
            self.server.job.as_ref().unwrap().abort();
            let _ = self.server.job.as_mut().unwrap().await;
        }
        self.server.job.take();
        drop(self.staging);
        let removed = fs::remove_dir_all(&self.cleanup.0);
        drop(self.cleanup);
        joined
            .expect("drain this fixture's own listener before removing its private root")
            .expect("fixture server task must exit")
            .expect("fixture listener must exit");
        removed.expect("remove only this exclusive synthetic manifest client directory");
    }
}
pub async fn checked<T, F>(f: Fixture, work: F) -> T
where
    F: for<'a> FnOnce(&'a Fixture) -> Pin<Box<dyn Future<Output = T> + 'a>>,
{
    let outcome = AssertUnwindSafe(work(&f)).catch_unwind().await;
    f.cleanup().await;
    match outcome {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
pub fn assert_request(
    request: &ObservedRequest,
    manifest: &FileTransferManifest,
    index: Option<u32>,
) {
    assert_eq!(request.method, "GET");
    let endpoint = if index.is_none() {
        "/platform/v1/file-manifests"
    } else {
        "/platform/v1/file-chunks"
    };
    assert_eq!(request.uri.path(), endpoint);
    let query = request.uri.query().unwrap();
    assert!(!query.contains('%') && !query.contains('+'));
    assert_eq!(
        query.split('&').count(),
        if index.is_none() { 6 } else { 7 }
    );
    let expected = ManifestQuery {
        grant_id: grant(),
        upload_id: upload(),
        file_id: manifest.file_id(),
        version_id: manifest.version_id(),
        byte_length: manifest.byte_length(),
        sha256: manifest.sha256().clone(),
    };
    match request.selected.as_ref().unwrap() {
        Selected::Manifest(query) => {
            assert!(index.is_none());
            assert_eq!(query, &expected);
        }
        Selected::Chunk(query) => {
            assert_eq!(Some(query.chunk_index), index);
            assert_eq!(
                ManifestQuery {
                    grant_id: query.grant_id,
                    upload_id: query.upload_id,
                    file_id: query.file_id,
                    version_id: query.version_id,
                    byte_length: query.byte_length,
                    sha256: query.sha256.clone()
                },
                expected
            );
        }
    }
    for (name, value) in [
        ("cookie", format!("cyanrex_platform_session={TOKEN}")),
        ("x-cyanrex-platform-request", "1".to_owned()),
        ("accept-encoding", "identity".to_owned()),
        (
            "accept",
            if index.is_none() {
                "application/json"
            } else {
                "application/octet-stream"
            }
            .to_owned(),
        ),
    ] {
        assert_eq!(request.headers.get_all(name).iter().count(), 1);
        assert_eq!(request.headers[name], value);
    }
    for name in [
        "origin",
        "authorization",
        "content-type",
        "range",
        "if-range",
        "if-match",
        "if-none-match",
    ] {
        assert!(
            !request.headers.contains_key(name),
            "controlled client must not invent {name}"
        );
    }
    assert!(request.body.is_empty());
}
