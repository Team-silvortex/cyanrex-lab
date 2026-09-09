struct LearningFeedbackFixture {
    state: std::sync::Arc<cyanrex_engine::AppState>,
    app: Router,
    path: std::path::PathBuf,
    attempt: Value,
    teacher_cookie: String,
    student_cookie: String,
}

impl LearningFeedbackFixture {
    async fn new() -> Self {
        let mut state = test_state();
        let path = std::env::temp_dir().join(format!(
            "cyanrex-learning-feedback-{}.json",
            uuid::Uuid::new_v4()
        ));
        std::sync::Arc::get_mut(&mut state).unwrap().learning_store =
            cyanrex_engine::services::learning_store::LearningStore::with_local_data_path(
                path.clone(),
            );
        let app = build_router(state.clone());
        let mut cookies = Vec::new();
        for username in ["teacher", "feedback-student"] {
            if username == "teacher" {
                state.auth_service.register(username, "feedback-pass-123").await.unwrap();
            } else {
                register_user(&app, username, "feedback-pass-123").await;
            }
            let otp = state
                .auth_service
                .generate_current_totp_for_user(username)
                .unwrap();
            cookies.push(login_for_user(&app, username, "feedback-pass-123", &otp).await);
        }
        let attempt = state
            .learning_store
            .record_run(
                "feedback-student",
                cyanrex_engine::services::learning_store::LearningRunOutcome {
                    lab_id: "01-first-program",
                    template_id: Some("xdp-pass"),
                    source: "SEC(\"xdp\") int pass(void *ctx) { return XDP_PASS; }",
                    run_success: true,
                    stage: "load",
                    attach_expected: false,
                    attach_verified: false,
                },
            )
            .await
            .unwrap();
        Self {
            state,
            app,
            path,
            attempt: serde_json::to_value(attempt).unwrap(),
            teacher_cookie: cookies[0].clone(),
            student_cookie: cookies[1].clone(),
        }
    }

    fn body(&self, comment: &str, expected_revision: u32) -> Value {
        serde_json::json!({
            "username": "feedback-student", "attempt_id": self.attempt["id"],
            "comment": comment, "expected_revision": expected_revision,
        })
    }

    async fn post(
        &self,
        cookie: Option<&str>,
        origin: Option<&str>,
        body: Value,
    ) -> axum::response::Response {
        let mut request = Request::builder()
            .method("POST")
            .uri("/learning/teacher/feedback")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        self.app
            .clone()
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap()
    }

    async fn get(&self, path: &str, cookie: &str) -> Value {
        let response = self
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
    }
}

impl Drop for LearningFeedbackFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[tokio::test]
async fn learning_teacher_feedback_is_saved_and_visible_to_its_student() {
    let fixture = LearningFeedbackFixture::new().await;
    let response = fixture
        .post(
            Some(&fixture.teacher_cookie),
            Some("http://localhost:3000"),
            fixture.body("  请补充边界检查。\nThen run the lab again.  ", 0),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let feedback: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(feedback["reviewer"], "teacher");
    assert_eq!(
        feedback["comment"],
        "请补充边界检查。\nThen run the lab again."
    );
    assert_eq!(feedback["revision"], 1);
    assert!(chrono::DateTime::parse_from_rfc3339(feedback["updated_at"].as_str().unwrap()).is_ok());

    let own = fixture
        .get("/learning/attempts", &fixture.student_cookie)
        .await;
    assert_eq!(own[0]["teacher_feedback"], feedback);
    for field in [
        "source",
        "source_sha256",
        "completed",
        "feedback",
        "created_at",
    ] {
        assert_eq!(
            own[0][field], fixture.attempt[field],
            "feedback must preserve {field}"
        );
    }
    let reviewed = fixture
        .get(
            "/learning/teacher/attempts?username=feedback-student",
            &fixture.teacher_cookie,
        )
        .await;
    assert_eq!(reviewed["attempts"][0]["teacher_feedback"], feedback);
    let other = fixture
        .get(
            "/learning/attempts?username=feedback-student",
            &fixture.teacher_cookie,
        )
        .await;
    assert_eq!(
        other,
        serde_json::json!([]),
        "own history must ignore a different username query"
    );
    let reloaded = cyanrex_engine::services::learning_store::LearningStore::with_local_data_path(
        fixture.path.clone(),
    );
    let persisted =
        serde_json::to_value(reloaded.attempts_for_user("feedback-student").await).unwrap();
    assert_eq!(persisted[0]["teacher_feedback"], feedback);
}

#[tokio::test]
async fn learning_teacher_feedback_enforces_staff_and_csrf() {
    let _env_guard = CSRF_ENV_LOCK.lock().await;
    let fixture = LearningFeedbackFixture::new().await;
    for (cookie, origin, expected) in [
        (
            None,
            Some("http://localhost:3000"),
            StatusCode::UNAUTHORIZED,
        ),
        (
            Some(fixture.student_cookie.as_str()),
            Some("http://localhost:3000"),
            StatusCode::FORBIDDEN,
        ),
        (
            Some(fixture.teacher_cookie.as_str()),
            Some("http://evil.example"),
            StatusCode::FORBIDDEN,
        ),
        (
            Some(fixture.teacher_cookie.as_str()),
            None,
            StatusCode::FORBIDDEN,
        ),
    ] {
        assert_eq!(
            fixture
                .post(cookie, origin, fixture.body("Review", 0))
                .await
                .status(),
            expected
        );
    }
    let own = fixture
        .get("/learning/attempts", &fixture.student_cookie)
        .await;
    assert!(own[0]["teacher_feedback"].is_null());
}

#[tokio::test]
async fn learning_teacher_feedback_validates_comment_and_attempt_owner() {
    let fixture = LearningFeedbackFixture::new().await;
    for comment in ["".to_string(), " \n\t ".to_string(), "好".repeat(2001)] {
        let response = fixture
            .post(
                Some(&fixture.teacher_cookie),
                Some("http://localhost:3000"),
                fixture.body(&comment, 0),
            )
            .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    for (field, value) in [
        ("username", "another-student".to_string()),
        ("attempt_id", uuid::Uuid::new_v4().to_string()),
    ] {
        let mut body = fixture.body("Review", 0);
        body[field] = Value::String(value);
        assert_eq!(
            fixture
                .post(
                    Some(&fixture.teacher_cookie),
                    Some("http://localhost:3000"),
                    body
                )
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    let mut body = fixture.body("Review", 0);
    body["username"] = serde_json::json!("teacher");
    assert_eq!(
        fixture
            .post(
                Some(&fixture.teacher_cookie),
                Some("http://localhost:3000"),
                body
            )
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let response = fixture
        .post(
            Some(&fixture.teacher_cookie),
            Some("http://localhost:3000"),
            fixture.body(&"好".repeat(2000), 0),
        )
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "limit counts Unicode characters, not UTF-8 bytes"
    );
}

#[tokio::test]
async fn learning_teacher_feedback_rejects_stale_and_concurrent_edits() {
    let fixture = LearningFeedbackFixture::new().await;
    let (first, second) = tokio::join!(
        fixture.post(
            Some(&fixture.teacher_cookie),
            Some("http://localhost:3000"),
            fixture.body("First", 0)
        ),
        fixture.post(
            Some(&fixture.teacher_cookie),
            Some("http://localhost:3000"),
            fixture.body("Second", 0)
        ),
    );
    let statuses = [first.status(), second.status()];
    assert!(statuses.contains(&StatusCode::OK));
    assert!(statuses.contains(&StatusCode::CONFLICT));
    let otp = fixture
        .state
        .auth_service
        .generate_current_totp_for_user("admin")
        .unwrap();
    let admin_cookie = login_and_get_session_cookie(&fixture.app, &otp).await;
    assert_eq!(
        fixture
            .post(
                Some(&admin_cookie),
                Some("http://localhost:3000"),
                fixture.body("Updated by admin", 1)
            )
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        fixture
            .post(
                Some(&fixture.teacher_cookie),
                Some("http://localhost:3000"),
                fixture.body("Stale", 1)
            )
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let own = fixture
        .get("/learning/attempts", &fixture.student_cookie)
        .await;
    assert_eq!(own[0]["teacher_feedback"]["comment"], "Updated by admin");
    assert_eq!(own[0]["teacher_feedback"]["reviewer"], "admin");
    assert_eq!(own[0]["teacher_feedback"]["revision"], 2);
}

#[tokio::test]
async fn learning_teacher_feedback_failure_does_not_publish_unsaved_changes() {
    let fixture = LearningFeedbackFixture::new().await;
    // A directory at the snapshot path forces the atomic rename to fail on every platform.
    tokio::fs::remove_file(&fixture.path).await.unwrap();
    tokio::fs::create_dir(&fixture.path).await.unwrap();
    let response = fixture
        .post(
            Some(&fixture.teacher_cookie),
            Some("http://localhost:3000"),
            fixture.body("Must not appear", 0),
        )
        .await;
    let own = fixture
        .get("/learning/attempts", &fixture.student_cookie)
        .await;
    tokio::fs::remove_dir(&fixture.path).await.unwrap();
    let _ = tokio::fs::remove_file(fixture.path.with_extension("json.tmp")).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert!(own[0]["teacher_feedback"].is_null());
    assert_eq!(
        fixture
            .post(
                Some(&fixture.teacher_cookie),
                Some("http://localhost:3000"),
                fixture.body("Retry", 0)
            )
            .await
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn learning_teacher_feedback_loads_legacy_attempts_without_feedback() {
    let fixture = LearningFeedbackFixture::new().await;
    let mut legacy = fixture.attempt.clone();
    legacy.as_object_mut().unwrap().remove("teacher_feedback");
    tokio::fs::write(&fixture.path, serde_json::to_vec(&vec![legacy]).unwrap())
        .await
        .unwrap();
    let reloaded = cyanrex_engine::services::learning_store::LearningStore::with_local_data_path(
        fixture.path.clone(),
    );
    let attempts = reloaded.attempts_for_user("feedback-student").await;
    assert_eq!(attempts.len(), 1);
    assert!(serde_json::to_value(&attempts[0]).unwrap()["teacher_feedback"].is_null());
}

#[tokio::test]
async fn learning_teacher_feedback_rejects_missing_revision_and_spoofed_author() {
    let fixture = LearningFeedbackFixture::new().await;
    let mut missing = fixture.body("Review", 0);
    missing.as_object_mut().unwrap().remove("expected_revision");
    let mut spoofed = fixture.body("Review", 0);
    spoofed["reviewer"] = serde_json::json!("admin");
    for body in [missing, spoofed] {
        assert_eq!(
            fixture
                .post(
                    Some(&fixture.teacher_cookie),
                    Some("http://localhost:3000"),
                    body
                )
                .await
                .status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let mut malformed = fixture.body("Review", 0);
    malformed["attempt_id"] = serde_json::json!("not-an-attempt-id");
    assert_eq!(
        fixture
            .post(
                Some(&fixture.teacher_cookie),
                Some("http://localhost:3000"),
                malformed
            )
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        fixture
            .post(
                Some(&fixture.teacher_cookie),
                Some("http://localhost:3000"),
                fixture.body("Review", u32::MAX)
            )
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let own = fixture
        .get("/learning/attempts", &fixture.student_cookie)
        .await;
    assert!(own[0]["teacher_feedback"].is_null());
}

#[tokio::test]
async fn learning_teacher_feedback_and_new_runs_preserve_both_updates() {
    let fixture = LearningFeedbackFixture::new().await;
    let (response, new_attempt) = tokio::join!(
        fixture.post(
            Some(&fixture.teacher_cookie),
            Some("http://localhost:3000"),
            fixture.body("Saved during another run", 0)
        ),
        fixture.state.learning_store.record_run(
            "feedback-student",
            cyanrex_engine::services::learning_store::LearningRunOutcome {
                lab_id: "01-first-program",
                template_id: Some("xdp-pass"),
                source: "a new failed attempt",
                run_success: false,
                stage: "compile",
                attach_expected: false,
                attach_verified: false,
            }
        ),
    );
    assert_eq!(response.status(), StatusCode::OK);
    let new_attempt = new_attempt.unwrap();
    let reloaded = cyanrex_engine::services::learning_store::LearningStore::with_local_data_path(
        fixture.path.clone(),
    );
    let attempts = reloaded.attempts_for_user("feedback-student").await;
    assert_eq!(attempts.len(), 2);
    assert!(attempts.iter().any(|attempt| attempt.id == new_attempt.id));
    let reviewed = attempts
        .iter()
        .find(|attempt| attempt.id == fixture.attempt["id"].as_str().unwrap())
        .unwrap();
    assert_eq!(
        reviewed.teacher_feedback.as_ref().unwrap().comment,
        "Saved during another run"
    );
}
