use super::*;
use cyanrex_engine::platform_http::TaskContentHttpConfigError;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Notify;

fn blocked_body(polled: Arc<Notify>) -> Body {
    Body::from_stream(futures_util::stream::poll_fn(move |_| {
        polled.notify_one();
        std::task::Poll::<Option<Result<axum::body::Bytes, std::io::Error>>>::Pending
    }))
}

fn counted_body(polled: Arc<AtomicBool>, text: String) -> Body {
    let mut bytes = Some(axum::body::Bytes::from(text));
    Body::from_stream(futures_util::stream::poll_fn(move |_| {
        polled.store(true, Ordering::SeqCst);
        std::task::Poll::Ready(bytes.take().map(Ok::<_, std::io::Error>))
    }))
}

fn valid_create() -> String {
    create_body(TASK.parse().unwrap(), &manifest("Capacity fixture", vec![]))
}

#[tokio::test]
async fn platform_http_default_four_slots_reject_fifth_before_body_or_storage() {
    let f = Offline::new().await;
    let mut held = Vec::new();
    let mut entered = true;
    for _ in 0..4 {
        let polled = Arc::new(Notify::new());
        held.push(tokio::spawn(f.app.clone().oneshot(request(
            "POST",
            COLLECTION,
            TOKEN,
            blocked_body(polled.clone()),
        ))));
        entered &= tokio::time::timeout(Duration::from_secs(1), polled.notified())
            .await
            .is_ok();
    }
    let polled = Arc::new(AtomicBool::new(false));
    let fifth = tokio::time::timeout(
        Duration::from_secs(1),
        f.app.clone().oneshot(request(
            "POST",
            COLLECTION,
            TOKEN,
            counted_body(polled.clone(), valid_create()),
        )),
    )
    .await;
    // Release every occupied body even when the unprotected implementation fails this Red.
    for handle in &held {
        handle.abort();
    }
    for handle in held {
        let _ = handle.await;
    }
    assert!(entered, "all four requests reached their body-read barrier");
    let response = fifth.expect("admission must not queue").unwrap();
    headers(&response);
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        body,
        json!({"schema_version":1,"error":{"code":"capacity_exhausted","outcome":"not_attempted"}})
    );
    assert!(
        !polled.load(Ordering::SeqCst),
        "full admission must not poll the fifth body"
    );
}

fn state(f: &Offline, capacity: usize) -> TaskContentHttpState {
    TaskContentHttpState::with_capacity(f.source.clone(), f.workspace.clone(), ORIGIN, capacity)
        .unwrap()
}

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
        let polled = Arc::new(Notify::new());
        let req = request("POST", COLLECTION, TOKEN, blocked_body(polled.clone()));
        let handle = tokio::spawn(async move { app.oneshot(req).await.unwrap() });
        let entered = tokio::time::timeout(Duration::from_secs(1), polled.notified())
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
        .expect("HTTP capacity admission must not queue")
        .unwrap()
}
async fn check_error(
    response: axum::response::Response,
    status: StatusCode,
    code: &str,
    outcome: &str,
) {
    headers(&response);
    assert_eq!(response.status(), status);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}
async fn exhausted(app: Router) {
    check_error(
        send(app, request("POST", COLLECTION, TOKEN, valid_create())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    )
    .await;
}
async fn available(app: Router) {
    // A closed source proves dispatch was reached; it must not look like saturation.
    check_error(
        send(app, request("POST", COLLECTION, TOKEN, valid_create())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
        "unconfirmed",
    )
    .await;
}

#[tokio::test]
async fn platform_http_capacity_configuration_is_explicit_and_bounded() {
    let f = Offline::new().await;
    for capacity in [0, 17, usize::MAX] {
        let result = TaskContentHttpState::with_capacity(
            f.source.clone(),
            f.workspace.clone(),
            ORIGIN,
            capacity,
        );
        assert!(matches!(
            result,
            Err(TaskContentHttpConfigError::InvalidCapacity)
        ));
    }
    for capacity in [1, 4, 16] {
        assert!(TaskContentHttpState::with_capacity(
            f.source.clone(),
            f.workspace.clone(),
            ORIGIN,
            capacity
        )
        .is_ok());
    }
    assert!(matches!(
        TaskContentHttpState::with_capacity(
            f.source.clone(),
            f.workspace.clone(),
            "http://untrusted.example",
            1
        ),
        Err(TaskContentHttpConfigError::InvalidOrigin)
    ));
}

#[tokio::test]
async fn platform_http_state_clones_and_builders_share_capacity_but_new_states_do_not() {
    let f = Offline::new().await;
    let shared = state(&f, 1);
    let first = build_task_content_router(shared.clone());
    let second = build_task_content_router(shared);
    let independent = build_task_content_router(state(&f, 1));
    let held = Held::start(first.clone()).await;
    assert!(held.entered);
    exhausted(first).await;
    exhausted(second.clone()).await;
    available(independent).await;
    held.cancel().await;
    available(second).await;
}

#[tokio::test]
async fn platform_http_saturation_preserves_path_and_credentials_precedence_without_polling() {
    let f = Offline::new().await;
    let app = build_task_content_router(state(&f, 1));
    let held = Held::start(app.clone()).await;
    assert!(held.entered);
    for mode in [
        "marker",
        "origin",
        "cookie",
        "authorization",
        "query",
        "path",
        "method",
        "valid",
    ] {
        let polled = Arc::new(AtomicBool::new(false));
        let mut req = request(
            "POST",
            COLLECTION,
            TOKEN,
            counted_body(polled.clone(), "not json".into()),
        );
        let (status, code) = match mode {
            "marker" => {
                req.headers_mut().remove(MARKER);
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "origin" => {
                req.headers_mut()
                    .insert("origin", "https://elsewhere.example".parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "cookie" => {
                req.headers_mut().remove("cookie");
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            "authorization" => {
                req.headers_mut()
                    .insert("authorization", "Bearer synthetic".parse().unwrap());
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            "query" => {
                *req.uri_mut() = format!("{COLLECTION}?unexpected=1").parse().unwrap();
                (StatusCode::BAD_REQUEST, "invalid_request")
            }
            "path" => {
                *req.uri_mut() = "/not-mounted".parse().unwrap();
                (StatusCode::NOT_FOUND, "not_found")
            }
            "method" => {
                *req.method_mut() = axum::http::Method::DELETE;
                (StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed")
            }
            _ => (StatusCode::SERVICE_UNAVAILABLE, "capacity_exhausted"),
        };
        check_error(send(app.clone(), req).await, status, code, "not_attempted").await;
        assert!(
            !polled.load(Ordering::SeqCst),
            "{mode}: admission must precede body consumption"
        );
    }
    held.cancel().await;
    available(app).await;
}

#[tokio::test]
async fn platform_http_cancelled_body_wait_releases_capacity() {
    let f = Offline::new().await;
    let app = build_task_content_router(state(&f, 1));
    let held = Held::start(app.clone()).await;
    assert!(held.entered);
    exhausted(app.clone()).await;
    held.cancel().await;
    available(app).await;
}

#[tokio::test]
async fn platform_http_body_deadline_keeps_slot_until_timeout_response_is_dropped() {
    let f = Offline::new().await;
    let app = build_task_content_router(state(&f, 1));
    let held = Held::start(app.clone()).await;
    assert!(held.entered);
    let response = held.timed_response().await;
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    headers(&response);
    exhausted(app.clone()).await;
    drop(response);
    available(app).await;
}

#[tokio::test]
async fn platform_http_body_and_parse_errors_hold_response_capacity_until_drop() {
    let f = Offline::new().await;
    let app = build_task_content_router(state(&f, 1));
    for mode in ["json", "media", "broken"] {
        let mut req = request("POST", COLLECTION, TOKEN, "not json");
        let status = match mode {
            "media" => {
                req.headers_mut().remove("content-type");
                StatusCode::UNSUPPORTED_MEDIA_TYPE
            }
            "broken" => {
                *req.body_mut() =
                    Body::from_stream(futures_util::stream::iter([Err::<axum::body::Bytes, _>(
                        std::io::Error::other("synthetic input failure"),
                    )]));
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::BAD_REQUEST,
        };
        let response = send(app.clone(), req).await;
        assert_eq!(response.status(), status);
        headers(&response);
        exhausted(app.clone()).await;
        drop(response);
        available(app.clone()).await;
    }
}

#[tokio::test]
async fn platform_http_dispatched_error_response_keeps_slot_after_headers_until_drop() {
    let f = Offline::new().await;
    let app = build_task_content_router(state(&f, 1));
    let response = send(
        app.clone(),
        request("POST", COLLECTION, TOKEN, valid_create()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    headers(&response);
    exhausted(app.clone()).await;
    drop(response);
    available(app).await;
}

#[tokio::test]
async fn platform_http_last_data_or_eof_releases_capacity_while_body_is_retained() {
    use axum::body::HttpBody;
    let f = Offline::new().await;
    let app = build_task_content_router(state(&f, 1));
    for poll_none in [false, true] {
        let response = send(
            app.clone(),
            request("POST", COLLECTION, TOKEN, valid_create()),
        )
        .await;
        let mut body = response.into_body();
        let last_data = body.frame().await.unwrap().unwrap().into_data().unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&last_data).unwrap()["error"]["outcome"],
            "unconfirmed"
        );
        assert!(
            body.is_end_stream(),
            "this JSON body ends with its single data frame"
        );
        if poll_none {
            assert!(body.frame().await.is_none());
        }
        // In the first mode no None frame was polled; both modes retain the Body allocation.
        available(app.clone()).await;
        assert!(body.is_end_stream());
        drop(body);
    }
}

#[tokio::test]
async fn platform_http_unpolled_request_future_does_not_reserve_capacity() {
    let f = Offline::new().await;
    let app = build_task_content_router(state(&f, 1));
    let polled = Arc::new(AtomicBool::new(false));
    let dormant = app.clone().oneshot(request(
        "POST",
        COLLECTION,
        TOKEN,
        counted_body(polled.clone(), valid_create()),
    ));
    available(app.clone()).await;
    drop(dormant);
    assert!(!polled.load(Ordering::SeqCst));
    available(app).await;
}
