#[tokio::test]
async fn learning_single_attempt_is_read_only_and_bound_to_the_session_owner() {
    let mut state = test_state();
    let directory = std::env::temp_dir().join(format!("cyanrex-resume-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("attempts.json");
    std::sync::Arc::get_mut(&mut state).unwrap().learning_store =
        cyanrex_engine::services::learning_store::LearningStore::with_local_data_path(path.clone());
    let app = build_router(state.clone());
    register_user(&app, "resume-student", "student-pass-123").await;
    let attempt = state
        .learning_store
        .record_run(
            "resume-student",
            cyanrex_engine::services::learning_store::LearningRunOutcome {
                lab_id: "01-first-program",
                template_id: Some("xdp-pass"),
                source: "// 原始提交\ninvalid source",
                run_success: false,
                stage: "compile",
                attach_expected: false,
                attach_verified: false,
            },
        )
        .await
        .unwrap();
    state
        .learning_store
        .save_teacher_feedback(
            "teacher",
            &cyanrex_engine::models::learning::SaveTeacherFeedbackRequest {
                username: "resume-student".into(),
                attempt_id: attempt.id.clone(),
                comment: "检查边界".into(),
                expected_revision: 0,
            },
        )
        .await
        .unwrap();
    let original = std::fs::read(&path).unwrap();
    let url = format!("/learning/attempt?attempt_id={}", attempt.id);
    let unauthorized = app
        .clone()
        .oneshot(Request::builder().uri(&url).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let otp = state
        .auth_service
        .generate_current_totp_for_user("resume-student")
        .unwrap();
    let cookie = login_for_user(&app, "resume-student", "student-pass-123", &otp).await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&url)
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let json: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(json["id"], attempt.id);
    assert_eq!(json["source"], attempt.source);
    assert_eq!(json["lab_id"], attempt.lab_id);
    assert_eq!(json["template_id"], "xdp-pass");
    assert_eq!(json["teacher_feedback"]["comment"], "检查边界");
    assert_eq!(json["teacher_feedback"]["revision"], 1);
    let admin_otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .unwrap();
    let admin_cookie = login_and_get_session_cookie(&app, &admin_otp).await;
    // Even staff cannot use this owner-only endpoint or a client-supplied username to adopt student source.
    for uri in [&url, &format!("{url}&username=resume-student")] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .header(header::COOKIE, &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let json: Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(json["message"], "attempt not found");
        assert!(json.get("source").is_none());
    }
    for (id, expected) in [
        ("not-an-id".to_string(), StatusCode::BAD_REQUEST),
        (uuid::Uuid::new_v4().to_string(), StatusCode::NOT_FOUND),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/learning/attempt?attempt_id={id}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "resuming must not update history or feedback"
    );
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&directory).unwrap();
}

#[tokio::test]
async fn learning_single_attempt_reports_storage_errors_without_exposing_source() {
    let mut state = test_state();
    let path = std::env::temp_dir().join(format!(
        "cyanrex-resume-corrupt-{}.json",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&path, "private malformed source").unwrap();
    std::sync::Arc::get_mut(&mut state).unwrap().learning_store =
        cyanrex_engine::services::learning_store::LearningStore::with_local_data_path(path.clone());
    let app = build_router(state.clone());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .unwrap();
    let cookie = login_and_get_session_cookie(&app, &otp).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/learning/attempt?attempt_id={}",
                    uuid::Uuid::new_v4()
                ))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let json: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(json["message"], "failed to load attempt");
    assert!(json.get("source").is_none());
    std::fs::remove_file(path).unwrap();
}
