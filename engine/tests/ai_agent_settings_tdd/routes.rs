use super::*;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn request(
    app: &Router,
    method: &str,
    cookie: Option<&str>,
    origin: Option<&str>,
    body: String,
    media: bool,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri("/settings/ai-agents");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    if media {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn live_route_is_teacher_only_csrf_checked_bounded_and_metadata_only() {
    let fixture = Fixture::new();
    std::env::set_var(
        "CYANREX_MODULES_DIR",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("modules"),
    );
    std::env::set_var("CYANREX_DATA_DIR", fixture.0.join("unused-root"));
    std::env::set_var("CYANREX_INSTANCE_ID", "ai-fixture");
    std::env::set_var("CYANREX_CORS_ORIGINS", "http://localhost:3000");
    std::env::set_var("CYANREX_ALLOW_MISSING_ORIGIN", "false");
    std::env::set_var("CYANREX_TEACHER_USERNAMES", "teacher");
    let mut state = cyanrex_engine::build_state();
    assert!(
        !fixture.0.exists(),
        "application startup must not create config storage"
    );
    std::sync::Arc::get_mut(&mut state)
        .unwrap()
        .ai_agent_settings = fixture.store();
    let mut cookies = Vec::new();
    for username in ["teacher", "student"] {
        state
            .auth_service
            .register(username, "fixture-only-password")
            .await
            .unwrap();
        let otp = state
            .auth_service
            .generate_current_totp_for_user(username)
            .unwrap();
        let login = state
            .auth_service
            .login(username, "fixture-only-password", &otp)
            .await
            .unwrap();
        cookies.push(format!("cyanrex_session={}", login.token));
    }
    let app = cyanrex_engine::build_router(state);
    let valid = serde_json::to_string(&update(0)).unwrap();
    for method in ["GET", "POST"] {
        for (cookie, expected) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some(cookies[1].as_str()), StatusCode::FORBIDDEN),
        ] {
            assert_eq!(
                request(
                    &app,
                    method,
                    cookie,
                    Some("http://localhost:3000"),
                    valid.clone(),
                    true
                )
                .await
                .0,
                expected
            );
        }
    }
    for origin in [None, Some("http://evil.example")] {
        assert_eq!(
            request(&app, "POST", Some(&cookies[0]), origin, valid.clone(), true)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    assert!(!fixture.0.exists());
    let read = request(&app, "GET", Some(&cookies[0]), None, String::new(), false).await;
    assert_eq!(
        read,
        (
            StatusCode::OK,
            json!({"revision":0,"default_profile_id":null,"profiles":[]})
        )
    );
    assert!(!fixture.0.exists());
    let write = request(
        &app,
        "POST",
        Some(&cookies[0]),
        Some("http://localhost:3000"),
        valid.clone(),
        true,
    )
    .await;
    assert_eq!(write.0, StatusCode::OK);
    assert_eq!(write.1["ok"], true);
    assert_eq!(write.1["settings"]["revision"], 1);
    assert_eq!(
        write.1["settings"]["profiles"][0]["credential_ref"],
        "LOCAL_MODEL_KEY"
    );
    assert_eq!(
        request(&app, "GET", Some(&cookies[0]), None, String::new(), false)
            .await
            .1,
        write.1["settings"]
    );
    assert_eq!(
        request(
            &app,
            "POST",
            Some(&cookies[0]),
            Some("http://localhost:3000"),
            valid,
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );

    let mut secret = serde_json::to_value(update(1)).unwrap();
    secret["profiles"][0]["api_key"] = json!("DO-NOT-ECHO-fixture-marker");
    for (body, media, status) in [
        (secret.to_string(), true, StatusCode::BAD_REQUEST),
        ("[1,null,[]]".into(), true, StatusCode::BAD_REQUEST),
        ("not-json".into(), true, StatusCode::BAD_REQUEST),
        ("{}".into(), false, StatusCode::UNSUPPORTED_MEDIA_TYPE),
        (
            format!(
                "{}{}",
                serde_json::to_string(&update(1)).unwrap(),
                " ".repeat(32768)
            ),
            true,
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ] {
        let response = request(
            &app,
            "POST",
            Some(&cookies[0]),
            Some("http://localhost:3000"),
            body,
            media,
        )
        .await;
        assert_eq!(response.0, status);
        assert!(!response.1.to_string().contains("DO-NOT-ECHO"));
    }
    let path = fixture.0.join("settings.json");
    let before = std::fs::read(&path).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&before).unwrap()["revision"],
        1
    );
    std::fs::write(&path, b"broken-DO-NOT-ECHO-fixture-marker").unwrap();
    for method in ["GET", "POST"] {
        let response = request(
            &app,
            method,
            Some(&cookies[0]),
            Some("http://localhost:3000"),
            serde_json::to_string(&update(1)).unwrap(),
            true,
        )
        .await;
        assert_eq!(response.0, StatusCode::SERVICE_UNAVAILABLE);
        assert!(!response.1.to_string().contains("DO-NOT-ECHO"));
        assert!(!response.1.to_string().contains(fixture.0.to_str().unwrap()));
    }
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"broken-DO-NOT-ECHO-fixture-marker"
    );

    // The configured layout owns both ai-agents and the instance directory, but
    // must not tighten or otherwise mutate the shared CYANREX_DATA_DIR parent.
    use std::os::unix::fs::PermissionsExt;
    let configured = AiAgentSettingsStore::from_env();
    configured.update(update(0)).await.unwrap();
    let root = fixture.0.join("unused-root");
    let parent = root.join("ai-agents");
    assert!(parent.join("ai-fixture/settings.json").exists());
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(configured.get().await.unwrap().revision, 1);
    std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        configured.get().await,
        Err(AiAgentSettingsError::StorageUnavailable)
    );
    assert_eq!(
        configured.update(update(1)).await,
        Err(AiAgentSettingsError::StorageUnavailable)
    );
    assert_eq!(
        std::fs::metadata(root).unwrap().permissions().mode() & 0o777,
        0o755
    );
}
