use super::*;
use axum::body::HttpBody;
use std::sync::Arc;
use tokio::sync::Notify;

fn blocked_body(polled: Arc<Notify>) -> Body {
    Body::from_stream(futures_util::stream::poll_fn(move |_| {
        polled.notify_one();
        std::task::Poll::<Option<Result<axum::body::Bytes, std::io::Error>>>::Pending
    }))
}
struct Held(Option<tokio::task::JoinHandle<axum::response::Response>>);
impl Drop for Held {
    fn drop(&mut self) {
        if let Some(handle) = &self.0 {
            handle.abort();
        }
    }
}
impl Held {
    async fn start(route: Router) -> Self {
        let entered = Arc::new(Notify::new());
        let req = request("GET", COLLECTION, blocked_body(entered.clone()));
        let held = Self(Some(tokio::spawn(async move {
            route.oneshot(req).await.unwrap()
        })));
        tokio::time::timeout(Duration::from_secs(1), entered.notified())
            .await
            .expect("admitted GET must reach its body barrier");
        held
    }
    async fn cancel(mut self) {
        let handle = self.0.take().unwrap();
        handle.abort();
        assert!(handle.await.unwrap_err().is_cancelled());
    }
}
async fn exhausted(route: Router) {
    tokio::time::timeout(
        Duration::from_secs(1),
        error(
            route,
            request("GET", COLLECTION, unread_body()),
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        ),
    )
    .await
    .expect("full budget must reject without queuing");
}

#[tokio::test]
async fn index_http_default_four_slots_reject_fifth_without_body_polling() {
    let f = Offline::new().await;
    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(Held::start(f.app.clone()).await);
    }
    exhausted(f.app.clone()).await;
    for pending in held {
        pending.cancel().await;
    }
    available(f.app).await;
}

#[tokio::test]
async fn index_http_clones_share_slots_through_response_eof_drop_and_cancellation() {
    let f = Offline::new().await;
    let state = f.state(1);
    let first = build_task_content_index_router(state.clone());
    let second = build_task_content_index_router(state);
    let independent = build_task_content_index_router(f.state(1));
    let held = Held::start(first.clone()).await;
    exhausted(first.clone()).await;
    exhausted(second.clone()).await;
    available(independent).await;
    held.cancel().await;
    let response = first
        .clone()
        .oneshot(request("GET", COLLECTION, Body::empty()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    exhausted(second.clone()).await;
    let mut body = response.into_body();
    let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&frame).unwrap(),
        json!({"schema_version":1,"error":{"code":"service_unavailable","outcome":"unconfirmed"}})
    );
    assert!(
        body.is_end_stream(),
        "EOF must retire the slot without requiring Body drop"
    );
    let response = second
        .clone()
        .oneshot(request("GET", COLLECTION, Body::empty()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    exhausted(first.clone()).await;
    drop(response);
    available(first).await;
    drop(body);
}

#[tokio::test]
async fn index_http_saturation_preserves_route_query_and_credential_precedence() {
    let f = Offline::new().await;
    let route = build_task_content_index_router(f.state(1));
    let held = Held::start(route.clone()).await;
    for mode in [
        "path",
        "method",
        "query",
        "marker",
        "origin",
        "cookie",
        "authorization",
        "valid",
    ] {
        let (method, path) = match mode {
            "path" => ("GET", "/platform/v1/task-index/unknown"),
            "method" => ("POST", COLLECTION),
            "query" => ("GET", "/platform/v1/task-index?owner=teacher"),
            _ => ("GET", COLLECTION),
        };
        let mut req = request(method, path, unread_body());
        let (status, code) = match mode {
            "path" => (StatusCode::NOT_FOUND, "not_found"),
            "method" => (StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed"),
            "query" => (StatusCode::BAD_REQUEST, "invalid_request"),
            "marker" => {
                req.headers_mut().remove(MARKER);
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "origin" => {
                req.headers_mut()
                    .insert("origin", "https://evil.example".parse().unwrap());
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
            _ => (StatusCode::SERVICE_UNAVAILABLE, "capacity_exhausted"),
        };
        tokio::time::timeout(
            Duration::from_secs(1),
            error(route.clone(), req, status, code, "not_attempted"),
        )
        .await
        .expect("cheap rejection must not wait behind admitted work");
    }
    held.cancel().await;
    available(route).await;
}
