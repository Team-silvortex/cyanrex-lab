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
