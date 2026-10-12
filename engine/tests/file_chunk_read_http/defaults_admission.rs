use crate::{fixture::*, offline::*, support::*, *};
use axum::body::HttpBody;
use std::{sync::Arc, time::Duration};
use tokio::sync::Notify;
struct Held(Option<tokio::task::JoinHandle<axum::response::Response>>);
impl Drop for Held {
    fn drop(&mut self) {
        if let Some(h) = &self.0 {
            h.abort();
        }
    }
}
impl Held {
    async fn start(route: Router) -> Self {
        let entered = Arc::new(Notify::new());
        let observer = entered.clone();
        let body = Body::from_stream(futures_util::stream::poll_fn(move |_| {
            observer.notify_one();
            std::task::Poll::<Option<Result<axum::body::Bytes, std::io::Error>>>::Pending
        }));
        let r = request_body(TOKEN, &uri(), body);
        let h = tokio::spawn(async move { route.oneshot(r).await.unwrap() });
        tokio::time::timeout(Duration::from_secs(1), entered.notified())
            .await
            .expect("admitted request must really reach its body poll");
        Self(Some(h))
    }
    async fn cancel(mut self) {
        let h = self.0.take().unwrap();
        h.abort();
        assert!(h.await.unwrap_err().is_cancelled());
    }
}
#[tokio::test]
async fn chunk_http_default_four_slots_and_clones_fail_fast_and_cancel_without_queuing() {
    let f = Offline::new().await;
    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(Held::start(f.app.clone()).await);
    }
    saturated(&f.app, &uri()).await;
    for h in held {
        h.cancel().await;
    }
    available(f.app.clone()).await;
    let state = f.state(1);
    let a = build_file_chunk_read_router(state.clone());
    let b = build_file_chunk_read_router(state);
    let independent = build_file_chunk_read_router(f.state(1));
    let held = Held::start(a.clone()).await;
    saturated(&a, &uri()).await;
    saturated(&b, &uri()).await;
    available(independent).await;
    held.cancel().await;
    available(b).await;
}
#[tokio::test]
async fn chunk_http_full_budget_preserves_endpoint_query_credential_precedence_without_body_polling(
) {
    let f = Offline::new().await;
    let route = build_file_chunk_read_router(f.state(1));
    let held = Held::start(route.clone()).await;
    for mode in [
        "path",
        "method",
        "query",
        "range",
        "marker",
        "origin",
        "cookie",
        "authorization",
        "valid",
    ] {
        let mut r = request_body(TOKEN, &uri(), unread());
        let (status, code) = match mode {
            "path" => {
                *r.uri_mut() = "/platform/v1/unknown".parse().unwrap();
                (StatusCode::NOT_FOUND, "not_found")
            }
            "method" => {
                *r.method_mut() = "POST".parse().unwrap();
                (StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed")
            }
            "query" => {
                *r.uri_mut() = format!("{}&owner=teacher", uri()).parse().unwrap();
                (StatusCode::BAD_REQUEST, "invalid_request")
            }
            "range" => {
                r.headers_mut()
                    .insert("range", "bytes=0-1".parse().unwrap());
                (StatusCode::BAD_REQUEST, "invalid_request")
            }
            "marker" => {
                r.headers_mut().remove(MARKER);
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "origin" => {
                r.headers_mut()
                    .insert("origin", "https://evil.example".parse().unwrap());
                (StatusCode::FORBIDDEN, "request_forbidden")
            }
            "cookie" => {
                r.headers_mut().remove("cookie");
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            "authorization" => {
                r.headers_mut()
                    .insert("authorization", "Bearer other".parse().unwrap());
                (StatusCode::UNAUTHORIZED, "invalid_session")
            }
            _ => (StatusCode::SERVICE_UNAVAILABLE, "capacity_exhausted"),
        };
        tokio::time::timeout(
            Duration::from_secs(1),
            error(route.clone(), r, status, code, "not_attempted"),
        )
        .await
        .unwrap();
    }
    held.cancel().await;
    available(route).await;
}
#[tokio::test]
async fn chunk_http_admitted_json_errors_hold_only_http_slots_until_eof_or_body_drop() {
    let f = Offline::new().await;
    let route = build_file_chunk_read_router(f.state(1));
    let response = route.clone().oneshot(request(TOKEN, &uri())).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    saturated(&route, &uri()).await;
    let mut body = response.into_body();
    let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&frame).unwrap(),
        serde_json::json!({"schema_version":1,"error":{"code":"service_unavailable","outcome":"unconfirmed"}})
    );
    assert!(body.is_end_stream());
    available(route.clone()).await;
    drop((body, frame));
    let response = route.clone().oneshot(request(TOKEN, &uri())).await.unwrap();
    saturated(&route, &uri()).await;
    drop(response);
    available(route).await;
}
