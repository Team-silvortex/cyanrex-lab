#[tokio::test]
async fn post_auth_student_can_access_ebpf_endpoints() {
    let state = test_state();
    let app = build_router(state.clone());

    register_user(&app, "alice", "alice-pass-123").await;
    let alice_otp = state
        .auth_service
        .generate_current_totp_for_user("alice")
        .expect("alice otp should exist");
    let session_cookie = login_for_user(&app, "alice", "alice-pass-123", &alice_otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ebpf/check")
                .header("content-type", "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, session_cookie)
                .body(Body::from(r#"{"code": "int main() { return 0; }"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(response.status(), StatusCode::FORBIDDEN);
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn post_auth_student_can_blocked_by_csrf_origin_restriction() {
    let state = test_state();
    let app = build_router(state.clone());

    register_user(&app, "alice", "alice-pass-123").await;
    let alice_otp = state
        .auth_service
        .generate_current_totp_for_user("alice")
        .expect("alice otp should exist");
    let session_cookie = login_for_user(&app, "alice", "alice-pass-123", &alice_otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/events/delete")
                .header("content-type", "application/json")
                .header(header::ORIGIN, "http://evil.example")
                .header(header::COOKIE, session_cookie)
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn post_auth_student_is_blocked_when_csrf_origin_is_missing() {
    let _env_guard = CSRF_ENV_LOCK.lock().await;
    let state = test_state();
    let app = build_router(state.clone());

    register_user(&app, "alice", "alice-pass-123").await;
    let alice_otp = state
        .auth_service
        .generate_current_totp_for_user("alice")
        .expect("alice otp should exist");
    let session_cookie = login_for_user(&app, "alice", "alice-pass-123", &alice_otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/events/delete")
                .header("content-type", "application/json")
                .header(header::COOKIE, session_cookie)
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn post_auth_student_is_allowed_when_csrf_origin_is_missing_with_override() {
    let _env_guard = CSRF_ENV_LOCK.lock().await;
    let previous = std::env::var("CYANREX_ALLOW_MISSING_ORIGIN").ok();
    std::env::set_var("CYANREX_ALLOW_MISSING_ORIGIN", "true");

    let state = test_state();
    let app = build_router(state.clone());

    register_user(&app, "alice", "alice-pass-123").await;
    let alice_otp = state
        .auth_service
        .generate_current_totp_for_user("alice")
        .expect("alice otp should exist");
    let session_cookie = login_for_user(&app, "alice", "alice-pass-123", &alice_otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/events/delete")
                .header("content-type", "application/json")
                .header(header::COOKIE, session_cookie)
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    match previous {
        Some(value) => std::env::set_var("CYANREX_ALLOW_MISSING_ORIGIN", value),
        None => std::env::remove_var("CYANREX_ALLOW_MISSING_ORIGIN"),
    }
}

#[tokio::test]
async fn post_auth_logout_is_blocked_by_csrf_origin_restriction() {
    let state = test_state();
    let app = build_router(state.clone());

    register_user(&app, "alice", "alice-pass-123").await;
    let alice_otp = state
        .auth_service
        .generate_current_totp_for_user("alice")
        .expect("alice otp should exist");
    let session_cookie = login_for_user(&app, "alice", "alice-pass-123", &alice_otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header(header::ORIGIN, "http://evil.example")
                .header(header::COOKIE, session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn post_auth_logout_allows_request_with_allowed_origin() {
    let state = test_state();
    let app = build_router(state.clone());

    register_user(&app, "alice", "alice-pass-123").await;
    let alice_otp = state
        .auth_service
        .generate_current_totp_for_user("alice")
        .expect("alice otp should exist");
    let session_cookie = login_for_user(&app, "alice", "alice-pass-123", &alice_otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn post_auth_student_is_forbidden_from_admin_settings_route() {
    let state = test_state();
    let app = build_router(state.clone());

    register_user(&app, "alice", "alice-pass-123").await;
    let alice_otp = state
        .auth_service
        .generate_current_totp_for_user("alice")
        .expect("alice otp should exist");
    let session_cookie = login_for_user(&app, "alice", "alice-pass-123", &alice_otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/settings/performance")
                .header(header::COOKIE, session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn post_auth_teacher_is_forbidden_from_admin_settings_route() {
    let state = test_state();
    let app = build_router(state.clone());
    register_user(&app, "teacher", "teacher-pass-123").await;
    let teacher_otp = state
        .auth_service
        .generate_current_totp_for_user("teacher")
        .expect("teacher otp should exist");
    let session_cookie = login_for_user(&app, "teacher", "teacher-pass-123", &teacher_otp).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/settings/performance")
                .header(header::COOKIE, session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
