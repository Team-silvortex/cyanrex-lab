//! Actual Source bytes cross one composed router; this proxy only records original client requests.
use super::*;
use cyanrex_engine::platform_http::{build_file_distribution_router, FileDistributionHttpState};
use std::{future::Future, io};

#[derive(Clone)]
pub struct DistributionBridge {
    router: Router,
    pub observed: Arc<Mutex<Vec<(Option<u32>, Vec<bytes::Entry>)>>>,
    statuses: Arc<Mutex<Vec<axum::http::StatusCode>>>,
    root: Arc<Mutex<PathBuf>>,
    pub pause: Arc<Mutex<Option<Arc<tokio::sync::Semaphore>>>>,
}
impl DistributionBridge {
    pub fn new(f: &chunk_fixture::Fixture, root: PathBuf) -> Self {
        let router = build_file_distribution_router(
            FileDistributionHttpState::new(
                f.sharing.base.grants.auth.source.clone(),
                f.workspace.clone(),
                "https://platform.example",
            )
            .unwrap(),
        );
        Self {
            router,
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
async fn forward(
    State(bridge): State<DistributionBridge>,
    request: Request<Body>,
) -> axum::response::Response {
    assert_eq!(request.method(), "GET");
    assert!(!request.headers().contains_key("origin"));
    let selectors = request
        .uri()
        .query()
        .unwrap()
        .split('&')
        .collect::<Vec<_>>();
    let index = match request.uri().path() {
        "/platform/v1/file-manifests" => {
            assert_eq!(selectors.len(), 6);
            None
        }
        "/platform/v1/file-chunks" => {
            assert_eq!(selectors.len(), 7);
            Some(
                selectors
                    .iter()
                    .find_map(|field| field.strip_prefix("chunk_index="))
                    .unwrap()
                    .parse::<u32>()
                    .unwrap(),
            )
        }
        _ => panic!("unexpected controlled client path"),
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
    // Do not choose independently configured routers by path: every original request reaches this one.
    let response = bridge.router.clone().oneshot(request).await.unwrap();
    bridge.statuses.lock().unwrap().push(response.status());
    response
}
pub struct DistributionServer {
    pub origin: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    job: Option<tokio::task::JoinHandle<io::Result<()>>>,
}
impl DistributionServer {
    pub async fn start(bridge: DistributionBridge) -> Self {
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
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let outcome =
            tokio::time::timeout(Duration::from_secs(5), self.job.as_mut().unwrap()).await;
        if outcome.is_err() {
            self.job.as_ref().unwrap().abort();
            let _ = tokio::time::timeout(Duration::from_secs(2), self.job.as_mut().unwrap()).await;
        }
        self.job.take();
        outcome
            .expect("drain the composed router's own listener before Source cleanup")
            .expect("distribution proxy task exits")
            .expect("distribution proxy listener exits");
    }
}
impl Drop for DistributionServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(job) = &self.job {
            job.abort();
        }
    }
}

pub async fn bounded<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .expect("bounded Source/distribution response operation")
}
