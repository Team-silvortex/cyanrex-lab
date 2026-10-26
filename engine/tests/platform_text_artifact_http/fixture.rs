pub(super) use super::retained_fixture::{
    artifact_id, revision_id, snapshot, token_hash, within, Fixture, Prepared,
};
use axum::{
    body::Body,
    http::{HeaderValue, Request, StatusCode},
    response::Response,
    Router,
};
use cyanrex_engine::platform_http::{
    build_text_artifact_publish_router, TextArtifactPublishHttpState,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

pub(super) const PATH: &str = "/platform/v1/text-artifacts";
pub(super) async fn router(p: &Prepared<'_>, capacity: usize) -> Router {
    build_text_artifact_publish_router(
        TextArtifactPublishHttpState::with_capacity(p.connector().await, capacity).unwrap(),
    )
}
pub(super) fn request(
    p: &Prepared<'_>,
    owner: usize,
    method: &str,
    path: &str,
    body: Body,
) -> Request<Body> {
    let mut cookie =
        HeaderValue::from_str(&format!("cyanrex_platform_session={}", p.tokens[owner])).unwrap();
    cookie.set_sensitive(true);
    Request::builder()
        .method(method)
        .uri(path)
        .header("origin", p.f.config().origin())
        .header("x-cyanrex-platform-request", "1")
        .header("cookie", cookie)
        .header("content-type", "application/json")
        .body(body)
        .unwrap()
}
pub(super) fn wire(n: u128, title: &str, text: &str) -> Body {
    Body::from(json!({"schema_version":1,"artifact_id":artifact_id(n),"revision_id":revision_id(n),"title":title,"text":text}).to_string())
}
pub(super) fn unread() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(
        |_| -> std::task::Poll<Option<Result<axum::body::Bytes, std::io::Error>>> {
            panic!("admission failure must not poll the request body")
        },
    ))
}
pub(super) fn headers(response: &Response) {
    for (key, expected) in [
        ("cache-control", "no-store"),
        ("pragma", "no-cache"),
        ("x-content-type-options", "nosniff"),
        ("content-type", "application/json"),
    ] {
        assert_eq!(response.headers()[key], expected);
    }
    for key in [
        "set-cookie",
        "retry-after",
        "location",
        "access-control-allow-origin",
    ] {
        assert!(!response.headers().contains_key(key));
    }
}
pub(super) async fn value(response: Response, status: StatusCode) -> Value {
    assert_eq!(response.status(), status);
    headers(&response);
    serde_json::from_slice(
        &within(response.into_body().collect())
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap()
}
pub(super) async fn call(route: &Router, request: Request<Body>, status: StatusCode) -> Value {
    value(
        within(route.clone().oneshot(request)).await.unwrap(),
        status,
    )
    .await
}
pub(super) async fn error(
    route: &Router,
    request: Request<Body>,
    status: StatusCode,
    code: &str,
    outcome: &str,
) {
    let response = within(route.clone().oneshot(request)).await.unwrap();
    if status == StatusCode::METHOD_NOT_ALLOWED {
        assert_eq!(response.headers()["allow"], "POST");
    }
    assert_eq!(
        value(response, status).await,
        json!({"schema_version":1,"error":{"code":code,"outcome":outcome}})
    );
}
