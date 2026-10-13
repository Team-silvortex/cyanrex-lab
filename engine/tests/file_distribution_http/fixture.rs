use crate::*;
use futures_util::FutureExt;
use std::{
    fs,
    future::Future,
    io,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    panic::AssertUnwindSafe,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

pub const ORIGIN: &str = "https://platform.example";
pub const TOKEN: &str = "ae0b35cd-c9b8-413c-a892-854b15959f41";
pub const COOKIE: &str = "cyanrex_platform_session";
pub const MARKER: &str = "x-cyanrex-platform-request";
// The durable collaboration fixture's original scope, not the legacy loopback client scope.
pub fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: "1223c6e9-823d-4cea-998a-a92f019b35fc".parse().unwrap(),
        workspace_id: "96699048-1d67-4ffd-b35f-ce22e287a4c7".parse().unwrap(),
    }
}
struct DirectoryGuard(PathBuf);
impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            if error.kind() != io::ErrorKind::NotFound {
                eprintln!("exclusive distribution HTTP fixture cleanup failed: {error}");
            }
        }
    }
}
pub struct Fixture {
    pub app: Router,
    pub state: FileDistributionHttpState,
    pub source: DurableAuthSource,
    pub workspace: SessionFileChunkReadWorkspace,
    pub pool: PgPool,
    pub root: PathBuf,
    pub manifest: FileTransferManifest,
    pub jobs: Arc<TaskTracker>,
    cleanup: DirectoryGuard,
}
impl Fixture {
    pub async fn new() -> Self {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://synthetic@127.0.0.1/unused")
            .unwrap();
        // The pool is closed before any Source command; neither test setup nor requests connect.
        pool.close().await;
        let source = DurableAuthSource::new(pool.clone(), scope().authority_id);
        let directory =
            std::env::temp_dir().join(format!("cyanrex-distribution-http-{}", Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let cleanup = DirectoryGuard(directory.clone());
        let root = directory.join("chunks");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let publication = FileUploadPublicationStore::new(
            pool.clone(),
            scope(),
            Uuid::new_v4(),
            "distribution_http_publications",
            16,
        )
        .unwrap();
        let recipients = source
            .file_recipient_grant_workspace(
                "distribution_http_recipients",
                scope(),
                Uuid::new_v4(),
                16,
            )
            .unwrap();
        let downloads = source
            .file_download_workspace(publication, recipients)
            .unwrap();
        let workspace = source
            .file_chunk_read_workspace(downloads, ChunkStaging::open(scope(), &root, 1).unwrap(), 1)
            .unwrap();
        let state =
            FileDistributionHttpState::new(source.clone(), workspace.clone(), ORIGIN).unwrap();
        let app = build_file_distribution_router(state.clone());
        Self {
            app,
            state,
            source,
            workspace,
            pool,
            root,
            manifest: bytes::manifest(17, 1),
            jobs: Arc::new(TaskTracker(Mutex::new(Vec::new()))),
            cleanup,
        }
    }
    pub fn path(&self, index: Option<u32>) -> String {
        let endpoint = if index.is_none() {
            "/platform/v1/file-manifests"
        } else {
            "/platform/v1/file-chunks"
        };
        let query = format!(
            "grant_id={}&upload_id={}&file_id={}&version_id={}&byte_length={}&sha256={}",
            Uuid::from_u128(101),
            Uuid::from_u128(201),
            self.manifest.file_id(),
            self.manifest.version_id(),
            self.manifest.byte_length(),
            self.manifest.sha256()
        );
        match index {
            Some(index) => format!("{endpoint}?{query}&chunk_index={index}"),
            None => format!("{endpoint}?{query}"),
        }
    }
    pub fn bounded(&self, manifests: usize, chunks: usize) -> FileDistributionHttpState {
        FileDistributionHttpState::with_capacities(
            self.source.clone(),
            self.workspace.clone(),
            ORIGIN,
            manifests,
            chunks,
        )
        .unwrap()
    }
    async fn cleanup(self) {
        let mut jobs = DrainingJobs(
            std::mem::take(&mut *self.jobs.0.lock().unwrap())
                .into_iter()
                .map(Some)
                .collect(),
        );
        for job in jobs.0.iter().flatten() {
            job.abort();
        }
        let mut drained = true;
        for job in &mut jobs.0 {
            let result = tokio::time::timeout(Duration::from_secs(3), job.as_mut().unwrap()).await;
            if result.is_ok() {
                job.take();
            } else {
                drained = false;
            }
        }
        let directory = self.cleanup.0.clone();
        // Drop both configured route states and their original staging descriptors before removal.
        drop(self);
        assert!(
            !directory.exists(),
            "checked cleanup must remove the exclusive private directory"
        );
        assert!(
            drained,
            "registered request tasks must terminate before cleanup assertions"
        );
    }
}
pub struct TaskTracker(pub Mutex<Vec<tokio::task::JoinHandle<axum::response::Response>>>);
impl Drop for TaskTracker {
    fn drop(&mut self) {
        for job in self.0.get_mut().unwrap().iter() {
            job.abort();
        }
    }
}
struct DrainingJobs(Vec<Option<tokio::task::JoinHandle<axum::response::Response>>>);
impl Drop for DrainingJobs {
    fn drop(&mut self) {
        for job in self.0.iter().flatten() {
            job.abort();
        }
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
pub fn request(path: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(path)
        .header(MARKER, "1")
        .header("origin", ORIGIN)
        .header("cookie", format!("{COOKIE}={TOKEN}"))
        .body(Body::empty())
        .unwrap()
}
pub struct ResponseObservation {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
pub async fn observe(app: &Router, request: Request<Body>) -> ResponseObservation {
    tokio::time::timeout(Duration::from_secs(5), async {
        let response = app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        ResponseObservation {
            status: parts.status,
            headers: parts.headers,
            body: body.collect().await.unwrap().to_bytes().to_vec(),
        }
    })
    .await
    .expect("bounded in-process route and real response-frame completion")
}
