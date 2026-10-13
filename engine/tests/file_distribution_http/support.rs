use crate::{fixture::*, *};
use axum::response::Response;
use std::{sync::Arc, time::Duration};
use tokio::sync::{Notify, Semaphore};

pub fn body_request(path: &str, body: Body) -> Request<Body> {
    let mut request = request(path);
    *request.body_mut() = body;
    request
}
pub fn unread() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("cheap or full-admission refusal must not poll this actual body")
        },
    ))
}
pub fn headers(headers: &HeaderMap) {
    for (name, value) in [
        ("content-type", "application/json"),
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
    ] {
        assert_eq!(headers[name], value);
    }
    for name in [
        "set-cookie",
        "location",
        "retry-after",
        "content-disposition",
        "content-range",
        "etag",
        "last-modified",
        "x-cyanrex-owner-id",
        "x-cyanrex-account-id",
        "x-cyanrex-source-key",
        "x-cyanrex-root-key",
        "x-cyanrex-journal-key",
        "x-cyanrex-file-transfer-version",
        "x-cyanrex-file-id",
        "x-cyanrex-file-version-id",
        "x-cyanrex-file-sha256",
        "x-cyanrex-chunk-index",
        "x-cyanrex-chunk-sha256",
    ] {
        assert!(
            !headers.contains_key(name),
            "private error exposes no {name}"
        );
    }
    assert!(headers
        .keys()
        .all(|name| !name.as_str().starts_with("access-control-")));
}
pub fn error_observed(
    response: ResponseObservation,
    status: StatusCode,
    code: &str,
    outcome: &str,
) {
    assert_eq!(response.status, status);
    headers(&response.headers);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&response.body).unwrap(),
        serde_json::json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}
pub async fn error(
    route: &Router,
    request: Request<Body>,
    status: StatusCode,
    code: &str,
    outcome: &str,
) {
    error_observed(observe(route, request).await, status, code, outcome);
}
pub async fn available(f: &Fixture, route: &Router, index: Option<u32>) {
    error(
        route,
        request(&f.path(index)),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}
pub async fn saturated(f: &Fixture, route: &Router, index: Option<u32>) {
    error(
        route,
        body_request(&f.path(index), unread()),
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    )
    .await;
}
pub async fn raw_response(route: &Router, request: Request<Body>) -> Response {
    tokio::time::timeout(Duration::from_secs(5), route.clone().oneshot(request))
        .await
        .expect("bounded response construction through original request/Source deadlines")
        .unwrap()
}
pub struct Held {
    future: Option<tokio::task::JoinHandle<Response>>,
    release: Arc<Semaphore>,
    tracker: Arc<TaskTracker>,
}
impl Held {
    pub async fn start(f: &Fixture, route: Router, index: Option<u32>) -> Self {
        let entered = Arc::new(Notify::new());
        let observer = entered.clone();
        let release = Arc::new(Semaphore::new(0));
        let gate = release.clone();
        let body = Body::from_stream(futures_util::stream::once(async move {
            observer.notify_one();
            gate.acquire().await.unwrap().forget();
            Ok::<_, std::io::Error>(axum::body::Bytes::new())
        }));
        let request = body_request(&f.path(index), body);
        let held = Self {
            future: Some(tokio::spawn(async move {
                route.oneshot(request).await.unwrap()
            })),
            release,
            tracker: f.jobs.clone(),
        };
        tokio::time::timeout(Duration::from_secs(1), entered.notified())
            .await
            .expect("actual admitted request reaches its empty-body barrier");
        held
    }
    pub async fn cancel(mut self) {
        self.future.as_ref().unwrap().abort();
        let result = tokio::time::timeout(Duration::from_secs(3), self.future.as_mut().unwrap())
            .await
            .expect("owned cancelled request task must terminate");
        self.future.take();
        assert!(result.unwrap_err().is_cancelled());
    }
    pub async fn finish(mut self) -> Response {
        self.release.add_permits(1);
        let result = tokio::time::timeout(Duration::from_secs(5), self.future.as_mut().unwrap())
            .await
            .expect("owned released request must return its closed Source error");
        self.future.take();
        result.unwrap()
    }
}
impl Drop for Held {
    fn drop(&mut self) {
        self.release.add_permits(1);
        if let Some(job) = self.future.take() {
            job.abort();
            self.tracker.0.lock().unwrap().push(job);
        }
    }
}
