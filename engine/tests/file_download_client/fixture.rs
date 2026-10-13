use crate::*;
use axum::{
    body::{Body, Bytes},
    extract::{Query, State},
    http::{HeaderMap, Method, Request, Response, StatusCode, Uri},
    Router,
};
use futures_util::FutureExt;
use http_body_util::BodyExt;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    future::Future,
    io::{self, Read},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

pub const TOKEN: &str = "2ba67287-cb7c-4850-a551-966104f77b35";
pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "ed990da2-85c5-48a2-a486-e598780f45d2".parse().unwrap(),
        workspace_id: "31cdde5b-7d03-44dc-9958-376132b23556".parse().unwrap(),
    }
}
pub fn grant() -> FileGrantId {
    Uuid::from_u128(101).try_into().unwrap()
}
pub fn upload() -> FileUploadId {
    Uuid::from_u128(201).try_into().unwrap()
}
pub fn chunk_name(manifest: &FileTransferManifest, index: u32) -> String {
    format!(
        "v1-{:x}-{index}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    )
}
pub fn output_name(manifest: &FileTransferManifest) -> String {
    format!(
        "f1-{:x}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    )
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RequestQuery {
    pub grant_id: FileGrantId,
    pub upload_id: FileUploadId,
    pub file_id: FileId,
    pub version_id: FileVersionId,
    pub byte_length: u64,
    pub sha256: Sha256Digest,
    pub chunk_index: u32,
}
#[derive(Debug)]
pub struct ObservedRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
    pub query: Result<RequestQuery, String>,
    pub staged_before_request: Vec<bytes::Entry>,
}
#[derive(Clone)]
struct ServerState {
    manifest: FileTransferManifest,
    root: PathBuf,
    observed: Arc<Mutex<Vec<ObservedRequest>>>,
    replies: Arc<Mutex<BTreeMap<u32, Reply>>>,
    produced: Arc<Mutex<BTreeMap<u32, usize>>>,
}
#[derive(Clone)]
pub struct Piece {
    pub data: Bytes,
    pub gate: Option<Arc<tokio::sync::Semaphore>>,
}
#[derive(Clone)]
pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub pieces: Vec<Piece>,
    pub header_gate: Option<Arc<tokio::sync::Semaphore>>,
}
impl Reply {
    pub fn good(manifest: &FileTransferManifest, index: u32) -> Self {
        let spec = manifest.chunk(index).unwrap();
        let data: Bytes = (spec.offset..spec.offset + spec.byte_length)
            .map(bytes::byte)
            .collect::<Vec<_>>()
            .into();
        Self {
            status: StatusCode::OK,
            headers: headers(manifest, index),
            pieces: vec![Piece { data, gate: None }],
            header_gate: None,
        }
    }
}
fn headers(manifest: &FileTransferManifest, index: u32) -> HeaderMap {
    let spec = manifest.chunk(index).unwrap();
    let mut headers = HeaderMap::new();
    for (name, value) in [
        ("content-length", spec.byte_length.to_string()),
        ("content-type", "application/octet-stream".to_owned()),
        (
            "content-disposition",
            "attachment; filename=\"chunk.bin\"".to_owned(),
        ),
        ("x-cyanrex-file-transfer-version", "1".to_owned()),
        ("x-cyanrex-file-id", manifest.file_id().to_string()),
        (
            "x-cyanrex-file-version-id",
            manifest.version_id().to_string(),
        ),
        ("x-cyanrex-chunk-index", index.to_string()),
        ("x-cyanrex-chunk-sha256", spec.sha256.to_string()),
        ("x-cyanrex-file-sha256", manifest.sha256().to_string()),
        ("cache-control", "no-store".to_owned()),
        ("pragma", "no-cache".to_owned()),
        ("x-content-type-options", "nosniff".to_owned()),
    ] {
        headers.insert(name, value.parse().unwrap());
    }
    headers
}
async fn handle(State(state): State<ServerState>, request: Request<Body>) -> Response<Body> {
    let (parts, body) = request.into_parts();
    let query = Query::<RequestQuery>::try_from_uri(&parts.uri)
        .map(|query| query.0)
        .map_err(|error| error.to_string());
    let index = query.as_ref().ok().map(|query| query.chunk_index);
    let body = body.collect().await.unwrap().to_bytes().to_vec();
    // Freeze controls before publishing the request event, including on a multithread runtime.
    let reply = index
        .filter(|index| state.manifest.chunk(*index).is_ok())
        .map(|index| {
            let reply = state
                .replies
                .lock()
                .unwrap()
                .get(&index)
                .cloned()
                .unwrap_or_else(|| Reply::good(&state.manifest, index));
            (index, reply)
        });
    let staged_before_request = bytes::tree(&state.root);
    state.observed.lock().unwrap().push(ObservedRequest {
        method: parts.method,
        uri: parts.uri,
        headers: parts.headers,
        body,
        query,
        staged_before_request,
    });
    let Some((index, reply)) = reply else {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::empty())
            .unwrap();
    };
    if let Some(gate) = reply.header_gate {
        gate.acquire().await.unwrap().forget();
    }
    let stream = futures_util::stream::unfold(
        (reply.pieces.into_iter(), state.produced),
        move |(mut pieces, produced)| async move {
            let piece = pieces.next()?;
            if let Some(gate) = piece.gate {
                gate.acquire().await.unwrap().forget();
            }
            *produced.lock().unwrap().entry(index).or_default() += 1;
            Some((Ok::<_, io::Error>(piece.data), (pieces, produced)))
        },
    );
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = reply.status;
    *response.headers_mut() = reply.headers;
    response
}

struct DirectoryGuard(PathBuf);
impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            if error.kind() != io::ErrorKind::NotFound {
                eprintln!("exclusive download fixture cleanup failed: {error}");
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
    replies: Arc<Mutex<BTreeMap<u32, Reply>>>,
    produced: Arc<Mutex<BTreeMap<u32, usize>>>,
    gates: Mutex<Vec<Arc<tokio::sync::Semaphore>>>,
    server: ServerGuard,
    cleanup: DirectoryGuard,
}
impl Fixture {
    pub async fn new(length: u64) -> Self {
        Self::with_manifest(bytes::manifest(length, 1)).await
    }
    pub async fn with_manifest(manifest: FileTransferManifest) -> Self {
        let directory =
            std::env::temp_dir().join(format!("cyanrex-download-client-{}", Uuid::new_v4()));
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
    pub fn client(&self, capacity: usize) -> FileChunkClient {
        FileChunkClient::with_config(
            &self.origin,
            self.staging.clone(),
            FileChunkClientConfig {
                transport: FileChunkTransport::LoopbackHttp,
                capacity,
                header_timeout: Duration::from_secs(1),
                idle_timeout: Duration::from_secs(1),
                total_timeout: Duration::from_secs(3),
                ..FileChunkClientConfig::default()
            },
        )
        .unwrap()
    }
    pub fn take_requests(&self) -> Vec<ObservedRequest> {
        std::mem::take(&mut *self.observed.lock().unwrap())
    }
    pub fn request_count(&self) -> usize {
        self.observed.lock().unwrap().len()
    }
    pub fn set_reply(&self, index: u32, reply: Reply) {
        let mut gates = self.gates.lock().unwrap();
        gates.extend(reply.header_gate.iter().cloned());
        gates.extend(reply.pieces.iter().filter_map(|piece| piece.gate.clone()));
        self.replies.lock().unwrap().insert(index, reply);
    }
    pub fn clear_reply(&self, index: u32) {
        self.replies.lock().unwrap().remove(&index);
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
    pub async fn wait_pieces(&self, index: u32, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self
                .produced
                .lock()
                .unwrap()
                .get(&index)
                .copied()
                .unwrap_or(0)
                < count
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("real loopback body must yield the selected piece");
    }
    pub async fn cleanup(mut self) {
        for gate in self.gates.into_inner().unwrap() {
            gate.add_permits(128);
        }
        let _ = self.server.stop.take().unwrap().send(());
        let mut server = self.server.job.take().unwrap();
        let joined = tokio::time::timeout(Duration::from_secs(5), &mut server).await;
        if joined.is_err() {
            server.abort();
            let _ = server.await;
        }
        drop(self.staging);
        let removed = fs::remove_dir_all(&self.cleanup.0);
        drop(self.cleanup);
        joined
            .expect("drain and await this fixture's own server")
            .expect("fixture server task must exit")
            .expect("fixture listener must exit");
        removed.expect("remove only this exclusive synthetic download root");
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

#[derive(Debug)]
pub struct Output {
    pub mode: u32,
    pub owner: u32,
    pub links: u64,
    pub length: u64,
    pub same_visible_inode: bool,
    pub verified: Result<(), String>,
    pub observed_length: u64,
    pub pattern_matches: bool,
    pub digest: String,
}
pub fn observe_output(path: &Path, manifest: &FileTransferManifest) -> io::Result<Output> {
    let visible = fs::symlink_metadata(path)?;
    if !visible.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "download output is not an ordinary file",
        ));
    }
    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    let verified = verify_file(manifest, &mut file).map_err(|error| error.to_string());
    // A second independent streamed oracle checks actual pattern, whole digest and exact EOF.
    let mut reader = fs::File::open(path)?.take(manifest.byte_length() + 1);
    let mut buffer = [0; FILE_TRANSFER_BUFFER_BYTES];
    let mut hash = Sha256::new();
    let mut offset = 0;
    let mut pattern_matches = true;
    loop {
        let amount = reader.read(&mut buffer)?;
        if amount == 0 {
            break;
        }
        for (position, value) in buffer[..amount].iter().enumerate() {
            pattern_matches &= *value == bytes::byte(offset + position as u64);
        }
        hash.update(&buffer[..amount]);
        offset += amount as u64;
    }
    Ok(Output {
        mode: metadata.mode(),
        owner: metadata.uid(),
        links: metadata.nlink(),
        length: metadata.len(),
        same_visible_inode: metadata.dev() == visible.dev() && metadata.ino() == visible.ino(),
        verified,
        observed_length: offset,
        pattern_matches,
        digest: format!("{:x}", hash.finalize()),
    })
}
