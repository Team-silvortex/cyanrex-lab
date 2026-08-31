async fn register_runner_agent(app: &Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runner/agent/register")
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {TEST_AGENT_TOKEN}"),
                )
                .body(Body::from(agent_registration_body()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    json["credential"].as_str().unwrap().to_string()
}

async fn signed_agent_post(
    app: &Router,
    credential: &str,
    path: &str,
    body: &str,
) -> axum::response::Response {
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let nonce = format!("nonce-{}", uuid::Uuid::new_v4().simple());
    let signature = agent_signature(
        credential,
        "POST",
        path,
        "lab-vm-01",
        &timestamp,
        &nonce,
        body.as_bytes(),
    );
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-cyanrex-agent-id", "lab-vm-01")
                .header("x-cyanrex-agent-timestamp", timestamp)
                .header("x-cyanrex-agent-nonce", nonce)
                .header("x-cyanrex-agent-signature", signature)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn mark_runner_agent_healthy(app: &Router, credential: &str) {
    let body = serde_json::json!({
        "agent_id": "lab-vm-01",
        "state": "healthy",
        "active_jobs": 0,
        "available_slots": 2,
        "kernel_release": "6.8.0-lab",
        "message": null
    })
    .to_string();
    let response = signed_agent_post(app, credential, "/runner/agent/heartbeat", &body).await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn runner_probe_job_supports_claim_cancel_sync_and_result() {
    let state = test_state();
    let app = build_router(state.clone());
    let credential = register_runner_agent(&app).await;
    mark_runner_agent_healthy(&app, &credential).await;

    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let cookie = login_and_get_session_cookie(&app, &otp).await;
    let submitted = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runner/jobs/probe")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({
                        "agent_id": "lab-vm-01",
                        "message": "control channel check",
                        "timeout_seconds": 30
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(submitted.status(), StatusCode::CREATED);
    let bytes = submitted.into_body().collect().await.unwrap().to_bytes();
    let submitted_json: Value = serde_json::from_slice(&bytes).unwrap();
    let job_id = submitted_json["job_id"].as_str().unwrap().to_string();
    assert_eq!(submitted_json["state"], "queued");

    let claim_body = serde_json::json!({"agent_id": "lab-vm-01"}).to_string();
    let claimed = signed_agent_post(
        &app,
        &credential,
        "/runner/agent/jobs/claim",
        &claim_body,
    )
    .await;
    assert_eq!(claimed.status(), StatusCode::OK);
    assert_eq!(
        claimed.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let bytes = claimed.into_body().collect().await.unwrap().to_bytes();
    let claimed_json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(claimed_json["job"]["job_id"], job_id);
    let lease_token = claimed_json["job"]["lease_token"]
        .as_str()
        .unwrap()
        .to_string();

    let cancelled = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runner/jobs/cancel")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({"job_id": job_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::OK);

    let sync_body = serde_json::json!({
        "agent_id": "lab-vm-01",
        "leases": [{"job_id": job_id, "lease_token": lease_token}]
    })
    .to_string();
    let synced = signed_agent_post(
        &app,
        &credential,
        "/runner/agent/jobs/sync",
        &sync_body,
    )
    .await;
    assert_eq!(synced.status(), StatusCode::OK);
    let bytes = synced.into_body().collect().await.unwrap().to_bytes();
    let synced_json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(synced_json["cancel_job_ids"][0], job_id);

    let result_body = serde_json::json!({
        "agent_id": "lab-vm-01",
        "job_id": job_id,
        "lease_token": lease_token,
        "state": "cancelled",
        "message": "cancel acknowledged",
        "output": null
    })
    .to_string();
    let result = signed_agent_post(
        &app,
        &credential,
        "/runner/agent/jobs/result",
        &result_body,
    )
    .await;
    assert_eq!(result.status(), StatusCode::OK);
    let bytes = result.into_body().collect().await.unwrap().to_bytes();
    let result_json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(result_json["state"], "cancelled");

    let inventory = app
        .oneshot(
            Request::builder()
                .uri("/runner/jobs")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(inventory.status(), StatusCode::OK);
    let bytes = inventory.into_body().collect().await.unwrap().to_bytes();
    let inventory_json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(inventory_json["total_jobs"], 1);
    assert_eq!(inventory_json["jobs"][0]["state"], "cancelled");
}

#[tokio::test]
async fn runner_agent_signed_requests_cannot_be_replayed() {
    let app = build_router(test_state());
    let credential = register_runner_agent(&app).await;
    mark_runner_agent_healthy(&app, &credential).await;
    let path = "/runner/agent/jobs/claim";
    let body = serde_json::json!({"agent_id": "lab-vm-01"}).to_string();
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let nonce = "nonce-replay-1234567890";
    let signature = agent_signature(
        &credential,
        "POST",
        path,
        "lab-vm-01",
        &timestamp,
        nonce,
        body.as_bytes(),
    );

    for expected in [StatusCode::OK, StatusCode::UNAUTHORIZED] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header("x-cyanrex-agent-id", "lab-vm-01")
                    .header("x-cyanrex-agent-timestamp", &timestamp)
                    .header("x-cyanrex-agent-nonce", nonce)
                    .header("x-cyanrex-agent-signature", &signature)
                    .body(Body::from(body.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[tokio::test]
async fn admin_can_submit_a_capability_matched_compile_check_without_inventory_source_leak() {
    let state = test_state();
    let app = build_router(state.clone());
    let credential = register_runner_agent(&app).await;
    mark_runner_agent_healthy(&app, &credential).await;
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .unwrap();
    let cookie = login_and_get_session_cookie(&app, &otp).await;
    let source = "#define VALUE 7\nint lesson(void) { return VALUE; }\n";
    let submitted = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runner/jobs/compile-check")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({
                        "agent_id": "lab-vm-01",
                        "source": source,
                        "program_name": "lesson",
                        "timeout_seconds": 20
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(submitted.status(), StatusCode::CREATED);
    let bytes = submitted.into_body().collect().await.unwrap().to_bytes();
    let submitted_json: Value = serde_json::from_slice(&bytes).unwrap();
    let job_id = submitted_json["job_id"].as_str().unwrap().to_string();
    assert_eq!(submitted_json["kind"], "ebpf_compile_check");
    assert_eq!(submitted_json["source_bytes"], source.len());
    assert!(submitted_json.get("source").is_none());

    let claim_body = serde_json::json!({"agent_id": "lab-vm-01"}).to_string();
    let claimed = signed_agent_post(
        &app,
        &credential,
        "/runner/agent/jobs/claim",
        &claim_body,
    )
    .await;
    assert_eq!(claimed.status(), StatusCode::OK);
    let bytes = claimed.into_body().collect().await.unwrap().to_bytes();
    let claimed_json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(claimed_json["job"]["source"], source);
    let lease_token = claimed_json["job"]["lease_token"].as_str().unwrap();
    let result_body = serde_json::json!({
        "agent_id": "lab-vm-01",
        "job_id": job_id,
        "lease_token": lease_token,
        "state": "succeeded",
        "message": "remote eBPF compile check passed",
        "output": "{\"success\":true,\"object_bytes\":512}"
    })
    .to_string();
    let result = signed_agent_post(
        &app,
        &credential,
        "/runner/agent/jobs/result",
        &result_body,
    )
    .await;
    assert_eq!(result.status(), StatusCode::OK);

    let inventory = app
        .oneshot(
            Request::builder()
                .uri("/runner/jobs")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = inventory.into_body().collect().await.unwrap().to_bytes();
    let inventory_json: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(inventory_json["jobs"][0].get("source").is_none());
    assert_eq!(inventory_json["jobs"][0]["source_bytes"], source.len());
}

#[tokio::test]
async fn authenticated_remote_check_is_user_scoped_and_normalizes_agent_diagnostics() {
    let state = test_state();
    let app = build_router(state.clone());
    let credential = register_runner_agent(&app).await;
    mark_runner_agent_healthy(&app, &credential).await;
    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .unwrap();
    let cookie = login_and_get_session_cookie(&app, &otp).await;

    let backends = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/ebpf/check/backends")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(backends.status(), StatusCode::OK);
    let body = backends.into_body().collect().await.unwrap().to_bytes();
    let backends_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(backends_json["agents"][0]["agent_id"], "lab-vm-01");

    let submit = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ebpf/check/remote")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({
                        "code": "int lesson(void) { return missing; }",
                        "agent_id": "lab-vm-01",
                        "program_name": "inline-check"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(submit.status(), StatusCode::ACCEPTED);
    let body = submit.into_body().collect().await.unwrap().to_bytes();
    let submit_json: Value = serde_json::from_slice(&body).unwrap();
    let job_id = submit_json["job_id"].as_str().unwrap().to_string();
    assert_eq!(submit_json["state"], "queued");

    let claim_body = serde_json::json!({"agent_id": "lab-vm-01"}).to_string();
    let claim = signed_agent_post(
        &app,
        &credential,
        "/runner/agent/jobs/claim",
        &claim_body,
    )
    .await;
    let body = claim.into_body().collect().await.unwrap().to_bytes();
    let claim_json: Value = serde_json::from_slice(&body).unwrap();
    let lease = claim_json["job"]["lease_token"].as_str().unwrap();
    let report = serde_json::json!({
        "success": false,
        "exit_code": 1,
        "timed_out": false,
        "stdout": "",
        "stdout_truncated": false,
        "stderr": "program.c:1:27: error: use of undeclared identifier 'missing'\n",
        "stderr_truncated": false,
        "object_bytes": null,
        "object_sha256": null,
        "duration_ms": 12
    })
    .to_string();
    let result_body = serde_json::json!({
        "agent_id": "lab-vm-01",
        "job_id": job_id,
        "lease_token": lease,
        "state": "failed",
        "message": "remote eBPF compile check failed",
        "output": report
    })
    .to_string();
    let result = signed_agent_post(
        &app,
        &credential,
        "/runner/agent/jobs/result",
        &result_body,
    )
    .await;
    assert_eq!(result.status(), StatusCode::OK);

    let status = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/ebpf/check/remote?job_id={job_id}"))
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let body = status.into_body().collect().await.unwrap().to_bytes();
    let status_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(status_json["state"], "failed");
    assert_eq!(status_json["result"]["diagnostics"][0]["line"], 1);

    let cancellable = state
        .runner_job_queue
        .submit_user_compile_check(
            "admin".to_string(),
            "lab-vm-01".to_string(),
            "int cancelled(void) { return 0; }".to_string(),
            None,
            None,
        )
        .unwrap();
    let cancelled = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/ebpf/check/remote/cancel")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::COOKIE, &cookie)
                .body(Body::from(
                    serde_json::json!({"job_id": cancellable.job_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::OK);
    let body = cancelled.into_body().collect().await.unwrap().to_bytes();
    let cancelled_json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(cancelled_json["state"], "cancelled");

    let hidden = state
        .runner_job_queue
        .submit_user_compile_check(
            "another-user".to_string(),
            "lab-vm-01".to_string(),
            "int hidden(void) { return 0; }".to_string(),
            None,
            None,
        )
        .unwrap();
    let forbidden_lookup = app
        .oneshot(
            Request::builder()
                .uri(format!("/ebpf/check/remote?job_id={}", hidden.job_id))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden_lookup.status(), StatusCode::NOT_FOUND);
}
