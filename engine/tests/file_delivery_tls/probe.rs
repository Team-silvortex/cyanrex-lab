//! Independent plain-HTTP redirect destination; hits count actual HTTP requests, not TCP accepts.
use axum::{body::Body, extract::State, http::Request, Router};
use std::{
    io,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
pub struct RedirectProbe {
    origin: String,
    hits: Arc<AtomicUsize>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    job: Option<tokio::task::JoinHandle<io::Result<()>>>,
}
async fn handle(State(hits): State<Arc<AtomicUsize>>, _request: Request<Body>) -> &'static str {
    hits.fetch_add(1, Ordering::SeqCst);
    "redirect target must never receive session requests"
}
impl RedirectProbe {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let app = Router::new().fallback(handle).with_state(hits.clone());
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let job = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
        });
        Self {
            origin,
            hits,
            stop: Some(stop),
            job: Some(job),
        }
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
    pub async fn finish(mut self) -> usize {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let joined = tokio::time::timeout(Duration::from_secs(5), self.job.as_mut().unwrap()).await;
        if joined.is_err() {
            self.job.as_ref().unwrap().abort();
            let _ = tokio::time::timeout(Duration::from_secs(2), self.job.as_mut().unwrap()).await;
        }
        self.job.take();
        joined
            .expect("bounded drain of independent redirect listener")
            .expect("redirect probe task exits")
            .expect("redirect probe listener exits");
        self.hits()
    }
}
impl Drop for RedirectProbe {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(job) = &self.job {
            job.abort();
        }
    }
}
