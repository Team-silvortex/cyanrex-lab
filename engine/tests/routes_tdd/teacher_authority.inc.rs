async fn authority_request(
    app: &Router,
    cookie: Option<&str>,
    method: &str,
    path: &str,
    body: Value,
    origin: &str,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, origin);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn teacher_authority_default_and_legacy_accounts_report_teacher() {
    let state = test_state();
    let app = build_router(state.clone());
    for username in ["admin", "teacher", "legacy-admin"] {
        let password = if username == "admin" { "cyanrex-admin" } else { "authority-pass-123" };
        if username != "admin" {
            // Trusted fixture provisioning, never a public registration of a reserved identity.
            state.auth_service.register(username, password).await.unwrap();
        }
        let otp = state.auth_service.generate_current_totp_for_user(username).unwrap();
        let cookie = login_for_user(&app, username, password, &otp).await;
        let response = authority_request(&app, Some(&cookie), "GET", "/auth/me", Value::Null, "http://localhost:3000").await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["username"], username);
        assert_eq!(json["role"], "teacher");
    }
}

#[tokio::test]
async fn teacher_authority_manages_deployment_and_retains_csrf() {
    let state = test_state();
    let app = build_router(state.clone());
    state.auth_service.register("teacher", "teacher-pass-123").await.unwrap();
    let otp = state.auth_service.generate_current_totp_for_user("teacher").unwrap();
    let cookie = login_for_user(&app, "teacher", "teacher-pass-123", &otp).await;
    for path in ["/settings/compiler", "/settings/performance", "/runner/overview", "/runner/agents", "/runner/jobs"] {
        let response = authority_request(&app, Some(&cookie), "GET", path, Value::Null, "http://localhost:3000").await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
    }
    for (path, body) in [
        ("/modules/start", serde_json::json!({"name": "module-network"})),
        ("/modules/stop", serde_json::json!({"name": "module-network"})),
        ("/command", serde_json::json!({"commandType": "ListModules"})),
    ] {
        let denied = authority_request(&app, Some(&cookie), "POST", path, body.clone(), "http://evil.example").await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN, "{path}");
        let allowed = authority_request(&app, Some(&cookie), "POST", path, body, "http://localhost:3000").await;
        assert_eq!(allowed.status(), StatusCode::OK, "{path}");
    }
}

#[tokio::test]
async fn teacher_authority_students_and_anonymous_cannot_manage_deployment() {
    let state = test_state();
    let app = build_router(state.clone());
    register_user(&app, "authority-student", "student-pass-123").await;
    let otp = state.auth_service.generate_current_totp_for_user("authority-student").unwrap();
    let cookie = login_for_user(&app, "authority-student", "student-pass-123", &otp).await;
    for (method, path) in [
        ("GET", "/settings/compiler"), ("POST", "/settings/compiler"),
        ("GET", "/settings/performance"), ("GET", "/runner/overview"),
        ("GET", "/runner/agents"), ("GET", "/runner/jobs"),
        ("POST", "/runner/jobs/probe"), ("POST", "/runner/jobs/compile-check"),
        ("POST", "/runner/jobs/cancel"), ("POST", "/modules/start"),
        ("POST", "/modules/stop"), ("POST", "/command"),
        ("POST", "/modules/c-headers/download"), ("POST", "/modules/c-headers/delete"),
        ("POST", "/modules/c-headers/select"),
    ] {
        for (cookie, expected) in [(Some(cookie.as_str()), StatusCode::FORBIDDEN), (None, StatusCode::UNAUTHORIZED)] {
            let response = authority_request(&app, cookie, method, path, serde_json::json!({}), "http://localhost:3000").await;
            assert_eq!(response.status(), expected, "{method} {path}");
        }
    }
}

#[tokio::test]
async fn teacher_authority_public_registration_cannot_claim_reserved_names_or_roles() {
    let state = test_state();
    let app = build_router(state.clone());
    for username in [" TEACHER ", "legacy-admin", "ADMIN"] {
        let response = authority_request(&app, None, "POST", "/auth/register",
            serde_json::json!({"username": username, "password": "untrusted-pass-123"}), "http://localhost:3000").await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{username}");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["ok"], false);
        assert!(json["secret"].is_null());
    }
    assert!(state.auth_service.generate_current_totp_for_user("teacher").is_none());
    assert!(state.auth_service.generate_current_totp_for_user("legacy-admin").is_none());
    let response = authority_request(&app, None, "POST", "/auth/register",
        serde_json::json!({"username": "self-appointed", "password": "student-pass-123", "role": "teacher", "admin": true}), "http://localhost:3000").await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(state.auth_service.role_for_username("self-appointed"), cyanrex_engine::models::auth::AuthRole::Student);
}

#[tokio::test]
async fn teacher_authority_deployment_owner_cannot_delete_itself_with_students_present() {
    let state = test_state();
    let app = build_router(state.clone());
    register_user(&app, "remaining-student", "student-pass-123").await;
    let otp = state.auth_service.generate_current_totp_for_user("admin").unwrap();
    let cookie = login_and_get_session_cookie(&app, &otp).await;
    let response = authority_request(&app, Some(&cookie), "POST", "/auth/delete",
        serde_json::json!({"password": "cyanrex-admin", "otp": otp}), "http://localhost:3000").await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let response = authority_request(&app, Some(&cookie), "GET", "/settings/compiler", Value::Null, "http://localhost:3000").await;
    assert_eq!(response.status(), StatusCode::OK);
}
