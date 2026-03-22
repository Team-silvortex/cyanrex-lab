use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use cyanrex_engine::{build_router, build_state};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn get_index_should_return_homepage_payload() {
    let app = build_router(build_state());

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["name"], "cyanrex-engine");
    assert_eq!(json["status"], "running");
}

#[tokio::test]
async fn get_health_should_return_ok_status() {
    let app = build_router(build_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn post_ebpf_run_with_empty_code_should_fail_validation() {
    let app = build_router(build_state());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ebpf/run")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"code": ""}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["success"], false);
    assert_eq!(json["stage"], "validation");
}

#[tokio::test]
async fn options_ebpf_run_should_allow_cors_preflight() {
    let app = build_router(build_state());

    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/ebpf/run")
                .header("origin", "http://localhost:3000")
                .header("access-control-request-method", "POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let allow_origin = response
        .headers()
        .get("access-control-allow-origin")
        .and_then(|value| value.to_str().ok());

    assert_eq!(allow_origin, Some("http://localhost:3000"));
}
