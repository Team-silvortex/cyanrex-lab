#[tokio::test]
async fn get_index_should_return_homepage_payload() {
    let app = build_router(test_state());

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
    let app = build_router(test_state());

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
    let state = test_state();
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
                .header(header::ORIGIN, "http://localhost:3000")
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
    let app = build_router(test_state());

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
async fn get_ebpf_templates_should_include_categorized_learning_paths() {
    let state = test_state();
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

    let mut has_learning_category = false;
    let mut has_learning_plus_category = false;

    for template in templates {
        let category = template["category"].as_str();
        if let Some(path) = category {
            assert!(!path.trim().is_empty());
            assert!(!path.ends_with('/'));
            assert!(!path.starts_with('/'));
            assert!(!path.contains("//"));
            let parts: Vec<_> = path.split('/').collect();
            assert!(parts.len() >= 2);
            if path.starts_with("learning/") {
                has_learning_category = true;
            }
            if path.starts_with("learning-plus/") {
                has_learning_plus_category = true;
            }
        }
    }

    assert!(has_learning_category);
    assert!(has_learning_plus_category);
}

#[tokio::test]
async fn post_ebpf_check_empty_code_should_return_validation_error() {
    let state = test_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let session_cookie = login_and_get_session_cookie(&app, &otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ebpf/check")
                .header("content-type", "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, session_cookie)
                .body(Body::from(r#"{"code": ""}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&payload).unwrap();

    assert_eq!(json["ok"], false);
    assert!(json["message"]
        .as_str()
        .unwrap_or("")
        .to_ascii_lowercase()
        .contains("empty"));
}

#[tokio::test]
async fn post_ebpf_check_with_oversized_code_should_return_payload_too_large() {
    let state = test_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let session_cookie = login_and_get_session_cookie(&app, &otp).await;

    let oversized = "a".repeat(262_145);
    let body = serde_json::json!({ "code": oversized }).to_string();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ebpf/check")
                .header("content-type", "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, session_cookie)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let payload = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&payload).unwrap();

    assert_eq!(json["ok"], false);
}

#[tokio::test]
async fn post_ebpf_run_with_oversized_code_should_fail_validation() {
    let state = test_state();
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
                .header(header::ORIGIN, "http://localhost:3000")
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
    let state = test_state();
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
    assert!(json["runtime_mode"].is_string());
    assert!(json["runtime_guidance"].is_string());
    assert!(json["checks"].is_array());
}

#[tokio::test]
async fn get_ebpf_templates_should_return_template_catalog() {
    let state = test_state();
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
    let state = test_state();
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
    let app = build_router(test_state());

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
    let state = test_state();
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
    let app = build_router(test_state());

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
    let app = build_router(test_state());

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
