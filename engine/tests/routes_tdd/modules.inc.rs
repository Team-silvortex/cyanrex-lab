#[tokio::test]
async fn get_c_headers_catalog_should_return_header_module_items() {
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
async fn get_modules_catalog_allowed_for_teacher() {
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
async fn post_modules_download_forbidden_for_teacher() {
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
                .method("POST")
                .uri("/modules/c-headers/download")
                .header("content-type", "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, session_cookie)
                .body(Body::from(r#"{"id":"nonexistent"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn get_modules_list_allowed_for_admin() {
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
                .uri("/modules")
                .header(header::COOKIE, session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&payload).unwrap();
    assert!(json.is_array());
}

#[tokio::test]
async fn get_modules_list_allowed_for_teacher() {
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
                .uri("/modules")
                .header(header::COOKIE, session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&payload).unwrap();
    assert!(json.is_array());
}

#[tokio::test]
async fn get_modules_list_forbidden_for_student() {
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
                .uri("/modules")
                .header(header::COOKIE, session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
