#[tokio::test]
async fn learning_labs_require_auth_and_return_progress_catalog() {
    let state = test_state();
    let app = build_router(state.clone());

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/learning/labs")
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
                .uri("/learning/labs")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let labs = json.as_array().expect("learning catalog should be an array");
    assert_eq!(labs.len(), 5);
    assert_eq!(labs[0]["lab"]["id"], "01-first-program");
    assert_eq!(labs[0]["status"], "not_started");
    assert_eq!(labs[0]["attempts"], 0);
}

#[tokio::test]
async fn teacher_learning_overview_is_staff_only() {
    let state = test_state();
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let admin_cookie = login_and_get_session_cookie(&app, &otp).await;
    let allowed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/learning/teacher/overview")
                .header(header::COOKIE, admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);

    register_user(&app, "learning-student", "student-pass-123").await;
    let student_otp = state
        .auth_service
        .generate_current_totp_for_user("learning-student")
        .expect("student otp should exist");
    let student_cookie = login_for_user(
        &app,
        "learning-student",
        "student-pass-123",
        &student_otp,
    )
    .await;
    let forbidden = app
        .oneshot(
            Request::builder()
                .uri("/learning/teacher/overview")
                .header(header::COOKIE, student_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
}
