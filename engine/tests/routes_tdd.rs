use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
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
        .clone()
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
    let state = build_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let session_cookie = login_and_get_session_cookie(&app, &otp).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ebpf/run")
                .header("content-type", "application/json")
                .header(header::COOKIE, &session_cookie)
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

    let events_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/events")
                .header(header::COOKIE, &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(events_response.status(), StatusCode::OK);
    let events_payload = events_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let events_json: Value = serde_json::from_slice(&events_payload).unwrap();
    let has_validation_event = events_json
        .as_array()
        .map(|events| {
            events
                .iter()
                .any(|event| event["event_type"] == "ebpf.validation_failed")
        })
        .unwrap_or(false);
    assert!(has_validation_event);
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
    let state = build_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let session_cookie = login_and_get_session_cookie(&app, &otp).await;

    let huge_code = "a".repeat(262_145);
    let body = serde_json::json!({ "code": huge_code }).to_string();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ebpf/run")
                .header("content-type", "application/json")
                .header(header::COOKIE, session_cookie)
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
    let state = build_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let session_cookie = login_and_get_session_cookie(&app, &otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/helper/environment")
                .header(header::COOKIE, session_cookie)
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
    let state = build_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let session_cookie = login_and_get_session_cookie(&app, &otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/modules/c-headers/catalog")
                .header(header::COOKIE, session_cookie)
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

#[tokio::test]
async fn get_ebpf_templates_should_return_template_catalog() {
    let state = build_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let session_cookie = login_and_get_session_cookie(&app, &otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ebpf/templates")
                .header(header::COOKIE, session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&payload).unwrap();
    let templates = json.as_array().expect("templates should be array");
    assert!(!templates.is_empty());
}

#[tokio::test]
async fn post_auth_login_should_succeed_with_valid_password_and_totp() {
    let state = build_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "admin",
                        "password": "cyanrex-admin",
                        "otp": otp,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get(header::SET_COOKIE).is_some());

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["username"], "admin");
}

#[tokio::test]
async fn post_auth_login_should_fail_with_invalid_totp() {
    let app = build_router(build_state());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"cyanrex-admin","otp":"000000"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], false);
}

#[tokio::test]
async fn get_auth_me_should_return_authenticated_after_login() {
    let state = build_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let session_cookie = login_and_get_session_cookie(&app, &otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/auth/me")
                .header(header::COOKIE, session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["authenticated"], true);
    assert_eq!(json["username"], "admin");
}

#[tokio::test]
async fn post_auth_totp_bootstrap_should_return_otpauth_uri_for_valid_credentials() {
    let app = build_router(build_state());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/bootstrap")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"cyanrex-admin"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["ok"], true);
    assert!(json["secret"].is_string());
    assert!(json["otpauth_uri"]
        .as_str()
        .unwrap_or_default()
        .starts_with("otpauth://totp/"));
}

#[tokio::test]
async fn post_auth_totp_bootstrap_should_fail_with_invalid_credentials() {
    let app = build_router(build_state());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/totp/bootstrap")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username":"admin","password":"wrong"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn post_auth_register_should_create_user_with_totp_bootstrap_payload() {
    let app = build_router(build_state());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"alice","password":"alice-pass-123"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["account_name"], "alice");
    assert!(json["secret"].is_string());
}

#[tokio::test]
async fn post_auth_change_password_should_require_valid_otp_and_update_login_password() {
    let state = build_state();
    let app = build_router(state.clone());

    register_user(&app, "alice", "alice-pass-123").await;

    let alice_otp = state
        .auth_service
        .generate_current_totp_for_user("alice")
        .expect("alice otp should exist");
    let alice_cookie = login_for_user(&app, "alice", "alice-pass-123", &alice_otp).await;

    let change_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/password/change")
                .header("content-type", "application/json")
                .header(header::COOKIE, alice_cookie)
                .body(Body::from(
                    serde_json::json!({
                        "current_password": "alice-pass-123",
                        "new_password": "alice-pass-456",
                        "otp": alice_otp,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(change_response.status(), StatusCode::OK);

    // old password should fail
    let old_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "alice",
                        "password": "alice-pass-123",
                        "otp": state.auth_service.generate_current_totp_for_user("alice").unwrap(),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);

    let new_login = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "alice",
                        "password": "alice-pass-456",
                        "otp": state.auth_service.generate_current_totp_for_user("alice").unwrap(),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(new_login.status(), StatusCode::OK);
}

#[tokio::test]
async fn post_auth_delete_should_remove_user_and_invalidate_login() {
    let state = build_state();
    let app = build_router(state.clone());

    register_user(&app, "alice", "alice-pass-123").await;

    let otp = state
        .auth_service
        .generate_current_totp_for_user("alice")
        .expect("alice otp should exist");
    let session_cookie = login_for_user(&app, "alice", "alice-pass-123", &otp).await;

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/delete")
                .header("content-type", "application/json")
                .header(header::COOKIE, session_cookie)
                .body(Body::from(
                    serde_json::json!({
                        "password": "alice-pass-123",
                        "otp": otp,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_response.status(), StatusCode::OK);

    let login_after_delete = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "alice",
                        "password": "alice-pass-123",
                        "otp": state.auth_service.generate_current_totp_for_user("alice").unwrap_or_else(|| "000000".to_string()),
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(login_after_delete.status(), StatusCode::UNAUTHORIZED);
}

async fn login_and_get_session_cookie(app: &Router, otp: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "admin",
                        "password": "cyanrex-admin",
                        "otp": otp,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .expect("set-cookie should exist")
        .to_string();

    let session_pair = set_cookie
        .split(';')
        .next()
        .expect("cookie pair should exist")
        .to_string();

    assert!(session_pair.starts_with("cyanrex_session="));
    session_pair
}

async fn register_user(app: &Router, username: &str, password: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": username,
                        "password": password,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn login_for_user(app: &Router, username: &str, password: &str, otp: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": username,
                        "password": password,
                        "otp": otp,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookie| cookie.split(';').next())
        .unwrap_or_default()
        .to_string()
}
