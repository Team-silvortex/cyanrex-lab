#[tokio::test]
async fn remote_check_cancellation_preserves_auth_csrf_and_owner_boundaries() {
    let _guard = CSRF_ENV_LOCK.lock().await;
    let state = test_state();
    let app = build_router(state.clone());
    let otp = state.auth_service.generate_current_totp_for_user("admin").unwrap();
    let cookie = login_and_get_session_cookie(&app, &otp).await;
    let own = state.runner_job_queue.submit_user_compile_check(
        "admin".into(), "compiler".into(), "int own(void) { return 1; }".into(), None, None).unwrap();
    let other = state.runner_job_queue.submit_user_compile_check(
        "other-owner".into(), "compiler".into(), "int other(void) { return 2; }".into(), None, None).unwrap();
    let managed = state.runner_job_queue.submit_compile_check(None,
        "int managed(void) { return 3; }".into(), None, None).unwrap();
    for (id, authenticated, origin, expected) in [
        (&own.job_id, false, "http://localhost:3000", StatusCode::UNAUTHORIZED),
        (&own.job_id, true, "https://untrusted.invalid", StatusCode::FORBIDDEN),
        (&other.job_id, true, "http://localhost:3000", StatusCode::NOT_FOUND),
        (&managed.job_id, true, "http://localhost:3000", StatusCode::NOT_FOUND),
    ] {
        let mut request = Request::builder().method("POST").uri("/ebpf/check/remote/cancel")
            .header(header::CONTENT_TYPE, "application/json").header(header::ORIGIN, origin);
        if authenticated { request = request.header(header::COOKIE, &cookie); }
        let response = app.clone().oneshot(request.body(Body::from(
            serde_json::json!({"job_id": id}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), expected);
        assert!(state.runner_job_queue.inventory().jobs.iter()
            .all(|job| job.state == cyanrex_engine::models::runner_job::RunnerJobState::Queued));
    }
    let response = app.oneshot(Request::builder().method("POST").uri("/ebpf/check/remote/cancel")
        .header(header::CONTENT_TYPE, "application/json").header(header::ORIGIN, "http://localhost:3000")
        .header(header::COOKIE, cookie)
        .body(Body::from(serde_json::json!({"job_id": own.job_id}).to_string())).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(state.runner_job_queue.job_for_owner(&own.job_id, "admin").unwrap().state,
        cyanrex_engine::models::runner_job::RunnerJobState::Cancelled);
    assert_eq!(state.runner_job_queue.job_for_owner(&other.job_id, "other-owner").unwrap().state,
        cyanrex_engine::models::runner_job::RunnerJobState::Queued);
}
