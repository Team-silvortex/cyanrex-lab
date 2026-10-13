use super::*;
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    io::{self, Read},
    path::Path,
    pin::Pin,
};

#[derive(Clone)]
pub struct Bridge {
    metadata: Router,
    chunks: Router,
    pub observed: Arc<Mutex<Vec<(Option<u32>, Vec<bytes::Entry>)>>>,
    statuses: Arc<Mutex<Vec<axum::http::StatusCode>>>,
    root: Arc<Mutex<PathBuf>>,
    pub pause: Arc<Mutex<Option<Arc<tokio::sync::Semaphore>>>>,
}
impl Bridge {
    pub fn new(f: &chunk_fixture::Fixture, root: PathBuf) -> Self {
        let source = &f.sharing.base.grants.auth.source;
        let metadata = source
            .file_download_workspace(
                f.sharing.base.publication.clone(),
                f.sharing.recipient_workspace.clone(),
            )
            .unwrap();
        Self {
            metadata: build_file_download_manifest_router(
                FileDownloadManifestHttpState::new(
                    source.clone(),
                    metadata,
                    "https://platform.example",
                )
                .unwrap(),
            ),
            chunks: build_file_chunk_read_router(
                FileChunkReadHttpState::new(
                    source.clone(),
                    f.workspace.clone(),
                    "https://platform.example",
                )
                .unwrap(),
            ),
            observed: Arc::new(Mutex::new(Vec::new())),
            statuses: Arc::new(Mutex::new(Vec::new())),
            root: Arc::new(Mutex::new(root)),
            pause: Arc::new(Mutex::new(None)),
        }
    }
    pub fn set_root(&self, root: PathBuf) {
        *self.root.lock().unwrap() = root;
    }
    pub fn take_requests(&self) -> Vec<(Option<u32>, Vec<bytes::Entry>)> {
        std::mem::take(&mut *self.observed.lock().unwrap())
    }
    pub fn take_statuses(&self) -> Vec<axum::http::StatusCode> {
        std::mem::take(&mut *self.statuses.lock().unwrap())
    }
}
async fn forward(State(bridge): State<Bridge>, request: Request<Body>) -> axum::response::Response {
    assert_eq!(request.method(), "GET");
    assert!(!request.headers().contains_key("origin"));
    let selectors = request
        .uri()
        .query()
        .unwrap()
        .split('&')
        .collect::<Vec<_>>();
    let (router, index) = match request.uri().path() {
        "/platform/v1/file-manifests" => {
            assert_eq!(selectors.len(), 6);
            (bridge.metadata.clone(), None)
        }
        "/platform/v1/file-chunks" => {
            assert_eq!(selectors.len(), 7);
            let index = selectors
                .iter()
                .find_map(|field| field.strip_prefix("chunk_index="))
                .unwrap()
                .parse::<u32>()
                .unwrap();
            (bridge.chunks.clone(), Some(index))
        }
        _ => panic!("unexpected controlled path"),
    };
    let pause = if index.is_some() {
        bridge.pause.lock().unwrap().clone()
    } else {
        None
    };
    bridge
        .observed
        .lock()
        .unwrap()
        .push((index, bytes::tree(&bridge.root.lock().unwrap())));
    if let Some(pause) = pause {
        pause.acquire().await.unwrap().forget();
    }
    // Each independent router owns a fallback. Dispatch by exact original path instead of merging.
    let response = router.oneshot(request).await.unwrap();
    bridge.statuses.lock().unwrap().push(response.status());
    response
}
pub struct Server {
    pub origin: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    job: Option<tokio::task::JoinHandle<io::Result<()>>>,
}
impl Server {
    pub async fn start(bridge: Bridge) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let app = Router::new().fallback(forward).with_state(bridge);
        let job = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
        });
        Self {
            origin,
            stop: Some(stop),
            job: Some(job),
        }
    }
    pub async fn stop(mut self) {
        let _ = self.stop.take().unwrap().send(());
        let outcome =
            tokio::time::timeout(Duration::from_secs(5), self.job.as_mut().unwrap()).await;
        if outcome.is_err() {
            self.job.as_ref().unwrap().abort();
            let _ = self.job.as_mut().unwrap().await;
        }
        self.job.take();
        outcome
            .expect("drain this fixture's own listener before Source cleanup")
            .expect("Source bridge task must exit")
            .expect("Source bridge listener must exit");
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(job) = &self.job {
            job.abort();
        }
    }
}
pub async fn checked<T, F>(f: chunk_fixture::Fixture, work: F) -> T
where
    F: for<'a> FnOnce(&'a chunk_fixture::Fixture) -> Pin<Box<dyn Future<Output = T> + 'a>>,
{
    let outcome = AssertUnwindSafe(work(&f)).catch_unwind().await;
    f.cleanup().await;
    match outcome {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
pub fn roots<const N: usize>(f: &chunk_fixture::Fixture, names: [&str; N]) -> [PathBuf; N] {
    names.map(|name| {
        let root = f.sharing.base.directory.join(name);
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        root
    })
}
pub async fn publish(f: &chunk_fixture::Fixture, seed: u128, length: u64) -> FileTransferManifest {
    let actor = f.sharing.actor();
    let manifest = f.sharing.base.stage_additional(seed, length, actor).await;
    let mut attempt = f
        .sharing
        .base
        .publisher
        .prepare(
            publication_fixture::upload(seed),
            actor.principal(),
            actor.account_id(),
        )
        .await
        .unwrap();
    let source = &f.sharing.base.grants.auth.source;
    let token = &f.sharing.base.grants.auth.learner_token;
    source
        .publish_session_file_upload(
            token,
            &f.sharing.base.workspace,
            grant_fixture::grant(1),
            &mut attempt,
        )
        .await
        .unwrap();
    source
        .issue_session_file_recipient_grant(
            token,
            &f.sharing.workspace,
            &SessionFileRecipientGrantIssue {
                grant_id: sharing_fixture::recipient_grant(seed),
                publisher_grant_id: grant_fixture::grant(1),
                upload_id: publication_fixture::upload(seed),
                expected_file: FileVersionRef::from_manifest(&manifest),
                recipient: f.sharing.recipient,
                validity: grant_fixture::validity(),
            },
        )
        .await
        .unwrap();
    manifest
}
pub fn assert_private_whole(root: &Path, manifest: &FileTransferManifest) {
    let key = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    );
    let path = root.join(format!("f1-{key}"));
    let mut file = fs::File::open(&path).unwrap();
    let opened = file.metadata().unwrap();
    let visible = fs::symlink_metadata(&path).unwrap();
    assert_eq!((opened.dev(), opened.ino()), (visible.dev(), visible.ino()));
    assert_eq!(opened.uid(), fs::metadata(root).unwrap().uid());
    assert_eq!(opened.mode() & 0o7777, 0o400);
    assert_eq!(opened.nlink(), 1);
    assert_eq!(opened.len(), manifest.byte_length());
    verify_file(manifest, &mut file).unwrap();
    let mut file = fs::File::open(&path).unwrap();
    let mut buffer = [0; FILE_TRANSFER_BUFFER_BYTES];
    let mut offset = 0;
    let mut hash = Sha256::new();
    loop {
        let amount = file.read(&mut buffer).unwrap();
        if amount == 0 {
            break;
        }
        for (index, value) in buffer[..amount].iter().enumerate() {
            assert_eq!(*value, bytes::byte(offset + index as u64));
        }
        hash.update(&buffer[..amount]);
        offset += amount as u64;
    }
    assert_eq!(offset, manifest.byte_length());
    assert_eq!(
        format!("{:x}", hash.finalize()),
        manifest.sha256().to_string()
    );
    for index in 0..manifest.chunk_sha256().len() as u32 {
        let mut file = fs::File::open(root.join(format!("v1-{key}-{index}"))).unwrap();
        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.mode() & 0o7777, 0o400);
        assert_eq!(metadata.nlink(), 1);
        assert_eq!(metadata.uid(), opened.uid());
        verify_chunk(manifest, index, &mut file).unwrap();
    }
    assert_eq!(bytes::tree(root).len(), manifest.chunk_sha256().len() + 1);
}

pub fn assert_saved_copy(
    parent: &Path,
    leaf: &str,
    source_root: &Path,
    manifest: &FileTransferManifest,
) {
    let path = parent.join(leaf);
    let visible = fs::symlink_metadata(&path).unwrap();
    let mut file = fs::File::open(&path).unwrap();
    let opened = file.metadata().unwrap();
    assert_eq!((opened.dev(), opened.ino()), (visible.dev(), visible.ino()));
    assert_eq!(opened.uid(), fs::metadata(parent).unwrap().uid());
    assert_eq!(opened.mode() & 0o7777, 0o400);
    assert_eq!(opened.nlink(), 1);
    assert_eq!(opened.len(), manifest.byte_length());
    let key = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(manifest).unwrap())
    );
    let original = fs::metadata(source_root.join(format!("f1-{key}"))).unwrap();
    assert_ne!(
        (opened.dev(), opened.ino()),
        (original.dev(), original.ino()),
        "selected output is a new actual file, not a rename or hardlink"
    );
    verify_file(manifest, &mut file).unwrap();
    let mut file = fs::File::open(&path).unwrap();
    let mut buffer = [0; FILE_TRANSFER_BUFFER_BYTES];
    let mut offset = 0;
    let mut hash = Sha256::new();
    loop {
        let amount = file.read(&mut buffer).unwrap();
        if amount == 0 {
            break;
        }
        for (index, value) in buffer[..amount].iter().enumerate() {
            assert_eq!(*value, bytes::byte(offset + index as u64));
        }
        hash.update(&buffer[..amount]);
        offset += amount as u64;
    }
    assert_eq!(offset, manifest.byte_length());
    assert_eq!(
        format!("{:x}", hash.finalize()),
        manifest.sha256().to_string()
    );
    let entries = bytes::tree(parent);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, std::ffi::OsString::from(leaf));
}
