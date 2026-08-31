use data_encoding::HEXLOWER;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

const TEST_AGENT_TOKEN: &str = "test-runner-agent-token-32-bytes-minimum";

fn agent_registration_body() -> String {
    serde_json::json!({
        "agent_id": "lab-vm-01",
        "protocol_version": 1,
        "agent_version": "0.2.0",
        "isolation": "virtual_machine",
        "max_concurrent": 2,
        "capabilities": ["bpftool", "btf", "clang_check", "ringbuf"],
        "labels": {"room": "a", "arch": "x86_64"}
    })
    .to_string()
}

fn agent_signature(
    credential: &str,
    method: &str,
    path: &str,
    agent_id: &str,
    timestamp: &str,
    nonce: &str,
    body: &[u8],
) -> String {
    let body_hash = HEXLOWER.encode(&Sha256::digest(body));
    let canonical = format!(
        "CYANREX-RUNNER-V1\n{method}\n{path}\n{agent_id}\n{timestamp}\n{nonce}\n{body_hash}"
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(credential.as_bytes()).unwrap();
    mac.update(canonical.as_bytes());
    HEXLOWER.encode(&mac.finalize().into_bytes())
}

#[tokio::test]
async fn runner_agent_registration_requires_bearer_token() {
    let app = build_router(test_state());

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runner/agent/register")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(agent_registration_body()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let inventory = app
        .oneshot(
            Request::builder()
                .uri("/runner/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(inventory.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn runner_agent_registration_rejects_oversized_payloads() {
    let app = build_router(test_state());
    let body = serde_json::json!({
        "agent_id": "lab-vm-oversized",
        "protocol_version": 1,
        "agent_version": "0.2.0",
        "isolation": "virtual_machine",
        "max_concurrent": 1,
        "capabilities": ["bpftool"],
        "labels": {"oversized": "x".repeat(70 * 1024)}
    });
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runner/agent/register")
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {TEST_AGENT_TOKEN}"),
                )
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn runner_agent_can_register_heartbeat_and_appear_in_admin_inventory() {
    let state = test_state();
    let app = build_router(state.clone());
    let authorization = format!("Bearer {TEST_AGENT_TOKEN}");

    let registered = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runner/agent/register")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, &authorization)
                .body(Body::from(agent_registration_body()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(registered.status(), StatusCode::OK);
    assert_eq!(
        registered.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let body = registered.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["agent_id"], "lab-vm-01");
    assert_eq!(json["state"], "healthy");
    assert_eq!(json["isolation"], "virtual_machine");
    assert_eq!(json["signature_scheme"], "hmac-sha256-v1");
    let credential = json["credential"].as_str().unwrap().to_string();
    assert_eq!(credential.len(), 64);

    let heartbeat_body = serde_json::json!({
        "agent_id": "lab-vm-01",
        "state": "degraded",
        "active_jobs": 1,
        "available_slots": 1,
        "kernel_release": "6.8.0-lab",
        "message": "classroom warmup"
    })
    .to_string();
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let nonce = format!("nonce-{}", uuid::Uuid::new_v4().simple());
    let signature = agent_signature(
        &credential,
        "POST",
        "/runner/agent/heartbeat",
        "lab-vm-01",
        &timestamp,
        &nonce,
        heartbeat_body.as_bytes(),
    );
    let heartbeat = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/runner/agent/heartbeat")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-cyanrex-agent-id", "lab-vm-01")
                .header("x-cyanrex-agent-timestamp", timestamp)
                .header("x-cyanrex-agent-nonce", nonce)
                .header("x-cyanrex-agent-signature", signature)
                .body(Body::from(heartbeat_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(heartbeat.status(), StatusCode::OK);

    let otp = state
        .auth_service
        .generate_current_totp_for_user("admin")
        .expect("default admin otp should be available");
    let cookie = login_and_get_session_cookie(&app, &otp).await;
    let inventory = app
        .oneshot(
            Request::builder()
                .uri("/runner/agents")
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(inventory.status(), StatusCode::OK);
    let body = inventory.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["enabled"], true);
    assert_eq!(json["total_agents"], 1);
    assert_eq!(json["online_agents"], 1);
    assert_eq!(json["agents"][0]["state"], "degraded");
    assert_eq!(json["agents"][0]["active_jobs"], 1);
}
