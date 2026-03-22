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

#[tokio::test]
async fn post_ebpf_run_with_oversized_code_should_fail_validation() {
    let app = build_router(build_state());
    let huge_code = "a".repeat(262_145);
    let body = serde_json::json!({ "code": huge_code }).to_string();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ebpf/run")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let payload = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&payload).unwrap();

    assert_eq!(json["stage"], "validation");
    assert_eq!(json["success"], false);
}

#[tokio::test]
async fn get_helper_environment_should_return_check_report() {
    let app = build_router(build_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/helper/environment")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let payload = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&payload).unwrap();

    assert!(json["overall_ok"].is_boolean());
    assert!(json["generated_at"].is_string());
    assert!(json["checks"].is_array());
}

#[tokio::test]
async fn get_c_headers_catalog_should_return_header_module_items() {
    let app = build_router(build_state());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/modules/c-headers/catalog")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let payload = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&payload).unwrap();

    assert!(json["headers"].is_array());
}
