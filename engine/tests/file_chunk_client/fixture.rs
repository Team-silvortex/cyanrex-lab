use super::*;
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, Method, Request, Response, StatusCode, Uri},
    Router,
};
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
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

#[derive(Clone, Debug)]
pub struct ObservedRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
#[derive(Clone)]
struct ServerState {
    reply: Arc<Mutex<Reply>>,
    observed: Arc<Mutex<Vec<ObservedRequest>>>,
    produced: Arc<AtomicUsize>,
}
#[derive(Clone)]
pub struct Piece {
    pub delay: Duration,
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
        let start = index as u64 * FILE_TRANSFER_CHUNK_BYTES;
        let chunk = manifest.chunk(index).unwrap();
        let data: Bytes = (start..start + chunk.byte_length)
            .map(bytes::byte)
            .collect::<Vec<_>>()
            .into();
        Self {
            status: StatusCode::OK,
            headers: headers(manifest, index),
            pieces: vec![Piece {
                delay: Duration::ZERO,
                data,
                gate: None,
            }],
            header_gate: None,
        }
    }
}
pub fn headers(manifest: &FileTransferManifest, index: u32) -> HeaderMap {
    let chunk = manifest.chunk(index).unwrap();
    let mut headers = HeaderMap::new();
    for (name, value) in [
        ("content-length", chunk.byte_length.to_string()),
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
        ("x-cyanrex-chunk-sha256", chunk.sha256.to_string()),
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
    let reply = state.reply.lock().unwrap().clone();
    let (parts, body) = request.into_parts();
    let body = body.collect().await.unwrap().to_bytes().to_vec();
    state.observed.lock().unwrap().push(ObservedRequest {
        method: parts.method,
        uri: parts.uri,
        headers: parts.headers,
        body,
    });
    if let Some(gate) = reply.header_gate {
        gate.acquire().await.unwrap().forget();
    }
    let stream = futures_util::stream::unfold(
        (reply.pieces.into_iter(), state.produced),
        |(mut pieces, produced)| async move {
            let piece = pieces.next()?;
            if let Some(gate) = piece.gate {
                gate.acquire().await.unwrap().forget();
            }
            if !piece.delay.is_zero() {
                tokio::time::sleep(piece.delay).await;
            }
            produced.fetch_add(1, Ordering::SeqCst);
            Some((Ok::<_, io::Error>(piece.data), (pieces, produced)))
        },
    );
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = reply.status;
    *response.headers_mut() = reply.headers;
    response
}

pub struct Fixture {
    pub origin: String,
    pub directory: PathBuf,
    pub root: PathBuf,
    pub staging: ChunkStaging,
    pub manifest: FileTransferManifest,
    observed: Arc<Mutex<Vec<ObservedRequest>>>,
    reply: Arc<Mutex<Reply>>,
    gates: Mutex<Vec<Arc<tokio::sync::Semaphore>>>,
    produced: Arc<AtomicUsize>,
    server: tokio::task::JoinHandle<()>,
}
impl Fixture {
    pub async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let directory =
            std::env::temp_dir().join(format!("cyanrex-chunk-client-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let root = directory.join("chunks");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let staging = ChunkStaging::open(scope(), &root, 1).unwrap();
        let manifest = bytes::manifest(17, 1);
        let observed = Arc::new(Mutex::new(Vec::new()));
        let reply = Arc::new(Mutex::new(Reply::good(&manifest, 0)));
        let produced = Arc::new(AtomicUsize::new(0));
        let state = ServerState {
            reply: reply.clone(),
            observed: observed.clone(),
            produced: produced.clone(),
        };
        let app = Router::new().fallback(handle).with_state(state);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            origin,
            directory,
            root,
            staging,
            manifest,
            observed,
            reply,
            gates: Mutex::new(Vec::new()),
            produced,
            server,
        }
    }
    pub fn observed(&self) -> Vec<ObservedRequest> {
        self.observed.lock().unwrap().clone()
    }
    pub fn set_reply(&self, reply: Reply) {
        let mut gates = self.gates.lock().unwrap();
        if let Some(g) = &reply.header_gate {
            gates.push(g.clone());
        }
        gates.extend(reply.pieces.iter().filter_map(|p| p.gate.clone()));
        *self.reply.lock().unwrap() = reply;
    }
    pub async fn wait_requests(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.observed().len() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("server must observe the actual request");
    }
    pub fn produced(&self) -> usize {
        self.produced.load(Ordering::SeqCst)
    }
    pub async fn wait_pieces(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.produced() < count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("real server must yield a data piece to transport");
    }
    pub async fn cleanup(self) {
        for gate in self.gates.into_inner().unwrap() {
            gate.add_permits(128);
        }
        self.server.abort();
        let _ = self.server.await;
        drop(self.staging);
        fs::remove_dir_all(&self.directory)
            .expect("remove only this exclusive synthetic client root");
    }
}
pub fn name(manifest: &FileTransferManifest) -> String {
    name_at(manifest, 0)
}
pub fn name_at(manifest: &FileTransferManifest, index: u32) -> String {
    format!(
        "v1-{:x}-{index}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    )
}
#[derive(Debug)]
pub struct Saved {
    pub mode: u32,
    pub length: u64,
    pub links: u64,
    pub same_inode: bool,
    pub verified: Result<(), String>,
    pub bytes: Vec<u8>,
}
pub fn saved(path: &Path, manifest: &FileTransferManifest) -> io::Result<Saved> {
    saved_at(path, manifest, 0)
}
pub fn saved_at(path: &Path, manifest: &FileTransferManifest, index: u32) -> io::Result<Saved> {
    let visible = fs::symlink_metadata(path)?;
    if !visible.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not an ordinary client file",
        ));
    }
    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    let verified = verify_chunk(manifest, index, &mut file).map_err(|error| error.to_string());
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(manifest.chunk(index).unwrap().byte_length + 1)
        .read_to_end(&mut bytes)?;
    Ok(Saved {
        mode: metadata.mode(),
        length: metadata.len(),
        links: metadata.nlink(),
        same_inode: metadata.dev() == visible.dev() && metadata.ino() == visible.ino(),
        verified,
        bytes,
    })
}
