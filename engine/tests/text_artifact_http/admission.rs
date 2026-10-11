use super::*;
use axum::body::HttpBody;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Notify;

struct Held {
    handle: Option<tokio::task::JoinHandle<axum::response::Response>>,
    entered: bool,
}
impl Drop for Held {
    fn drop(&mut self) {
        if let Some(handle) = &self.handle {
            handle.abort();
        }
    }
}
impl Held {
    async fn start(app: Router) -> Self {
        let notified = Arc::new(Notify::new());
        let polled = notified.clone();
        let body = Body::from_stream(futures_util::stream::poll_fn(move |_| {
            polled.notify_one();
            std::task::Poll::<Option<Result<axum::body::Bytes, std::io::Error>>>::Pending
        }));
        let handle = tokio::spawn(async move {
            app.oneshot(request("POST", TEXT_COLLECTION, TOKEN, body))
                .await
                .unwrap()
        });
        let entered = tokio::time::timeout(Duration::from_secs(1), notified.notified())
            .await
            .is_ok();
        Self {
            handle: Some(handle),
            entered,
        }
    }
    async fn cancel(mut self) {
        let handle = self.handle.take().unwrap();
        handle.abort();
        let _ = handle.await;
    }
    async fn timed_response(mut self) -> axum::response::Response {
        let response = tokio::time::timeout(Duration::from_secs(3), self.handle.as_mut().unwrap())
            .await
            .expect("bounded body must finish")
            .unwrap();
        self.handle.take();
        response
    }
}
async fn send(app: Router, req: Request<Body>) -> axum::response::Response {
    tokio::time::timeout(Duration::from_secs(1), app.oneshot(req))
        .await
        .expect("admission never queues")
        .unwrap()
}
async fn exhausted(app: Router) {
    tokio::time::timeout(
        Duration::from_secs(1),
        error(
            app,
            request("POST", TEXT_COLLECTION, TOKEN, valid_body("")),
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        ),
    )
    .await
    .expect("capacity must fail immediately");
}
async fn available(app: Router) {
    dispatched(app, request("POST", TEXT_COLLECTION, TOKEN, valid_body(""))).await;
}

#[tokio::test]
async fn text_artifact_http_capacity_shares_only_cloned_text_state_budgets() {
    let f = Offline::new().await;
    let mut held = Vec::new();
    for _ in 0..4 {
        let waiting = Held::start(f.app.clone()).await;
        assert!(waiting.entered);
        held.push(waiting);
    }
    exhausted(f.app.clone()).await;
    for waiting in held {
        waiting.cancel().await;
    }
    available(f.app.clone()).await;

    let state = f.state(1);
    let first = build_text_artifact_router(state.clone());
    let second = build_text_artifact_router(state);
    let separate = build_text_artifact_router(f.state(1));
    let waiting = Held::start(first).await;
    assert!(waiting.entered);
    exhausted(second.clone()).await;
    available(separate).await;
    let workspace =
        SessionTaskContentWorkspace::new("private_tasks", scope(), f.workspace.clone()).unwrap();
    let tasks = build_task_content_router(
        TaskContentHttpState::with_capacity(f.source.clone(), workspace, ORIGIN, 1).unwrap(),
    );
    error(
        tasks,
        request(
            "POST",
            http_fixture::COLLECTION,
            TOKEN,
            http_fixture::create_body(id(), &manifest("Separate budget", vec![])),
        ),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
    waiting.cancel().await;
    available(second).await;
}

#[tokio::test]
async fn text_artifact_http_saturation_precedes_body_and_cancellation_releases_admission() {
    let f = Offline::new().await;
    let app = build_text_artifact_router(f.state(1));
    let waiting = Held::start(app.clone()).await;
    assert!(waiting.entered);
    for mode in ["marker", "origin", "cookie", "valid"] {
        let polled = Arc::new(AtomicBool::new(false));
        let counter = polled.clone();
        let mut bytes = Some(axum::body::Bytes::from_static(b"invalid JSON"));
        let body = Body::from_stream(futures_util::stream::poll_fn(move |_| {
            counter.store(true, Ordering::SeqCst);
            std::task::Poll::Ready(bytes.take().map(Ok::<_, std::io::Error>))
        }));
        let mut req = request("POST", TEXT_COLLECTION, TOKEN, body);
        let (status, code) = match mode {
            "marker" => {
                req.headers_mut().remove(MARKER);
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "origin" => {
                req.headers_mut()
                    .insert("origin", "https://other.example".parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "cookie" => {
                req.headers_mut().remove("cookie");
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            _ => (StatusCode::SERVICE_UNAVAILABLE, "capacity_exhausted"),
        };
        tokio::time::timeout(
            Duration::from_secs(1),
            error(app.clone(), req, status, code, "not_attempted"),
        )
        .await
        .expect("saturation cannot queue");
        assert!(
            !polled.load(Ordering::SeqCst),
            "{mode}: headers and capacity precede body consumption"
        );
    }
    waiting.cancel().await;
    available(app.clone()).await;
    // A merely constructed, never-polled future cannot reserve another slot.
    let dormant = app
        .clone()
        .oneshot(request("POST", TEXT_COLLECTION, TOKEN, valid_body("")));
    available(app).await;
    drop(dormant);
}

#[tokio::test]
async fn text_artifact_http_response_body_owns_capacity_through_error_timeout_and_eof() {
    let f = Offline::new().await;
    let app = build_text_artifact_router(f.state(1));
    for mode in ["json", "source", "timeout", "last_data", "eof"] {
        let response = if mode == "timeout" {
            let waiting = Held::start(app.clone()).await;
            assert!(waiting.entered);
            waiting.timed_response().await
        } else {
            send(
                app.clone(),
                request(
                    "POST",
                    TEXT_COLLECTION,
                    TOKEN,
                    if mode == "json" {
                        "{}".into()
                    } else {
                        valid_body("")
                    },
                ),
            )
            .await
        };
        headers(&response);
        assert_eq!(
            response.status(),
            match mode {
                "json" => StatusCode::BAD_REQUEST,
                "timeout" => StatusCode::REQUEST_TIMEOUT,
                _ => StatusCode::SERVICE_UNAVAILABLE,
            }
        );
        exhausted(app.clone()).await;
        if matches!(mode, "last_data" | "eof") {
            let mut body = response.into_body();
            let data = body.frame().await.unwrap().unwrap().into_data().unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&data).unwrap()["error"]["outcome"],
                "unconfirmed"
            );
            assert!(body.is_end_stream());
            if mode == "eof" {
                assert!(body.frame().await.is_none());
            }
            available(app.clone()).await;
            assert!(body.is_end_stream());
            drop(body);
        } else {
            drop(response);
            available(app.clone()).await;
        }
    }
}
