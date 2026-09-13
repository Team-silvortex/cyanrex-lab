async fn runner_capacity_heartbeat(app: &Router, credential: &str, active: u16, available: u16) {
    let body = serde_json::json!({
        "agent_id": "lab-vm-01", "state": "healthy",
        "active_jobs": active, "available_slots": available,
        "kernel_release": null, "message": null,
    }).to_string();
    let response = signed_agent_post(app, credential, "/runner/agent/heartbeat", &body).await;
    assert_eq!(response.status(), StatusCode::OK);
}

async fn runner_capacity_claim(app: &Router, credential: &str) -> (StatusCode, Value) {
    let response = signed_agent_post(app, credential, "/runner/agent/jobs/claim",
        r#"{"agent_id":"lab-vm-01"}"#).await;
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn runner_claim_does_not_subtract_reported_active_jobs_twice() {
    let state = test_state();
    let app = build_router(state.clone());
    let credential = register_runner_agent(&app).await;
    for index in 0..3 {
        state.runner_job_queue.submit_probe(None, format!("probe-{index}"), None).unwrap();
    }
    runner_capacity_heartbeat(&app, &credential, 0, 2).await;
    let (status, first) = runner_capacity_claim(&app, &credential).await;
    assert_eq!(status, StatusCode::OK);
    assert!(first["job"].is_object());
    // The heartbeat's free slot already excludes the first outstanding lease.
    runner_capacity_heartbeat(&app, &credential, 1, 1).await;
    let (status, second) = runner_capacity_claim(&app, &credential).await;
    assert_eq!(status, StatusCode::OK);
    assert!(second["job"].is_object(), "one free slot must admit the second lease");
    assert_ne!(first["job"]["job_id"], second["job"]["job_id"]);
    // A stale heartbeat must not allow another claim beyond the total usable capacity.
    assert!(runner_capacity_claim(&app, &credential).await.1["job"].is_null());
    runner_capacity_heartbeat(&app, &credential, 2, 0).await;
    assert_eq!(runner_capacity_claim(&app, &credential).await.0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn runner_claim_keeps_reserved_capacity_and_cancel_pending_leases_counted() {
    let state = test_state();
    let app = build_router(state.clone());
    let credential = register_runner_agent(&app).await;
    for index in 0..3 {
        state.runner_job_queue.submit_probe(None, format!("probe-{index}"), None).unwrap();
    }
    // Registered max is two, but the Agent currently offers only one usable slot.
    runner_capacity_heartbeat(&app, &credential, 0, 1).await;
    let first = runner_capacity_claim(&app, &credential).await.1;
    assert!(first["job"].is_object());
    let job_id = first["job"]["job_id"].as_str().unwrap();
    state.runner_job_queue.cancel(job_id).unwrap();
    assert!(runner_capacity_claim(&app, &credential).await.1["job"].is_null());
    state.runner_job_queue.complete(cyanrex_engine::models::runner_job::RunnerJobResultRequest {
        agent_id: "lab-vm-01".into(), job_id: job_id.into(),
        lease_token: first["job"]["lease_token"].as_str().unwrap().into(),
        state: cyanrex_engine::models::runner_job::RunnerJobResultState::Cancelled,
        message: None, output: None,
    }).unwrap();
    assert!(runner_capacity_claim(&app, &credential).await.1["job"].is_object());
}
