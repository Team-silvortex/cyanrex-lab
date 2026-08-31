#[tokio::test]
async fn runner_status_requires_auth_and_reports_shared_kernel_boundary() {
    let state = test_state();
    let app = build_router(state.clone());

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/runner/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let cookie = login_and_get_session_cookie(&app, &otp).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/runner/status")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["mode"], "local_process");
    assert_eq!(json["isolation"], "shared_kernel");
    assert_eq!(json["active_total"], 0);
    assert_eq!(json["active_for_current_user"], 0);
    assert!(json["max_concurrent"].as_u64().unwrap_or_default() >= 1);
    assert!(json["execution_timeout_seconds"]
        .as_u64()
        .unwrap_or_default()
        >= 5);
}

#[tokio::test]
async fn runner_overview_is_admin_only() {
    let state = test_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let admin_cookie = login_and_get_session_cookie(&app, &otp).await;

    for path in ["/runner/overview", "/runner/agents", "/runner/jobs"] {
        let allowed = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(allowed.status(), StatusCode::OK, "admin path: {path}");
    }

    register_user(&app, "runner-student", "student-pass-123").await;
    let student_otp = state
        .auth_service
        .generate_current_totp_for_user("runner-student")
        .expect("student otp should exist");
    let student_cookie = login_for_user(
        &app,
        "runner-student",
        "student-pass-123",
        &student_otp,
    )
    .await;
    for path in ["/runner/overview", "/runner/agents", "/runner/jobs"] {
        let forbidden = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::COOKIE, &student_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN, "student path: {path}");
    }
}
