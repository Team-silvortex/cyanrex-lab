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

#[tokio::test]
async fn teacher_attempt_review_is_limited_and_students_cannot_read_it() {
    let mut state = test_state();
    let data_path = std::env::temp_dir().join(format!(
        "cyanrex-learning-review-{}.json",
        uuid::Uuid::new_v4()
    ));
    std::sync::Arc::get_mut(&mut state)
        .expect("test state should be uniquely owned")
        .learning_store =
        cyanrex_engine::services::learning_store::LearningStore::with_local_data_path(
            data_path.clone(),
        );
    let app = build_router(state.clone());
    register_user(&app, "review-student", "student-pass-123").await;

    for source in [
        "SEC(\"xdp\") int first(void *ctx) { return XDP_PASS; }",
        "SEC(\"xdp\") int second(void *ctx) { return XDP_PASS; }",
    ] {
        state
            .learning_store
            .record_run(
                "review-student",
                cyanrex_engine::services::learning_store::LearningRunOutcome {
                    lab_id: "01-first-program",
                    template_id: Some("xdp-pass"),
                    source,
                    run_success: true,
                    stage: "load",
                    attach_expected: false,
                    attach_verified: false,
                },
            )
            .await
            .expect("learning attempt should be recorded");
    }

    let admin_otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let admin_cookie = login_and_get_session_cookie(&app, &admin_otp).await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/learning/teacher/attempts?username=review-student&limit=1")
                .header(header::COOKIE, admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["username"], "review-student");
    assert_eq!(json["attempts"].as_array().unwrap().len(), 1);
    assert!(json["attempts"][0]["source"]
        .as_str()
        .unwrap()
        .contains("XDP_PASS"));

    let student_otp = state
        .auth_service
        .generate_current_totp_for_user("review-student")
        .expect("student otp should exist");
    let student_cookie = login_for_user(
        &app,
        "review-student",
        "student-pass-123",
        &student_otp,
    )
    .await;
    let forbidden = app
        .oneshot(
            Request::builder()
                .uri("/learning/teacher/attempts?username=review-student")
                .header(header::COOKIE, student_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    let _ = tokio::fs::remove_file(data_path).await;
}
