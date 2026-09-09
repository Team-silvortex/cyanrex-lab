use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use cyanrex_engine::{build_router, build_state, services::classroom::ClassroomService};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::{Arc, Once};
use tower::ServiceExt;

const CLASSROOM_ID: &str = "23d40d83-19de-43d3-85fe-506d86592a70";
const ORIGIN: &str = "http://localhost:3000";

fn fixture(enabled: bool) -> (Router, Arc<cyanrex_engine::AppState>) {
    static ENV: Once = Once::new();
    ENV.call_once(|| {
        std::env::set_var("CYANREX_ALLOW_REGISTRATION", "false");
        std::env::set_var("CYANREX_ALLOW_TOTP_BOOTSTRAP", "false");
        std::env::set_var("CYANREX_CORS_ORIGINS", ORIGIN);
        std::env::set_var("CYANREX_TEACHER_USERNAMES", "teacher");
        std::env::set_var("CYANREX_ADMIN_USERNAMES", "legacy-admin");
        std::env::set_var("CYANREX_ALLOW_MISSING_ORIGIN", "false");
    });
    let mut state = build_state();
    Arc::get_mut(&mut state).unwrap().classroom = if enabled {
        ClassroomService::configured(CLASSROOM_ID, "Lab A", ORIGIN).unwrap()
    } else {
        ClassroomService::default()
    };
    (build_router(state.clone()), state)
}

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    origin: &str,
    body: Value,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if !origin.is_empty() {
        req = req.header(header::ORIGIN, origin);
    }
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    if response.headers().contains_key(header::CONTENT_TYPE)
        && status != StatusCode::UNPROCESSABLE_ENTITY
        && status != StatusCode::PAYLOAD_TOO_LARGE
    {
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
    }
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn cookie(state: &cyanrex_engine::AppState, username: &str) -> String {
    if username != "admin" {
        state
            .auth_service
            .register(username, "test-password-123")
            .await
            .unwrap();
    }
    let otp = state
        .auth_service
        .generate_current_totp_for_user(username)
        .unwrap();
    let login = state
        .auth_service
        .login(
            username,
            if username == "admin" {
                "cyanrex-admin"
            } else {
                "test-password-123"
            },
            &otp,
        )
        .await
        .unwrap();
    format!("cyanrex_session={}", login.token)
}

async fn invite(app: &Router, teacher: &str, username: &str) -> Value {
    let (status, body) = request(
        app,
        "POST",
        "/classroom/invitations",
        Some(teacher),
        ORIGIN,
        json!({"username": username}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

fn join_body(invite: &Value) -> Value {
    let url = reqwest::Url::parse(invite["join_url"].as_str().unwrap()).unwrap();
    assert!(
        url.query().is_none(),
        "invitation must not be sent in URL query logs"
    );
    let fragment =
        reqwest::Url::parse(&format!("http://localhost/?{}", url.fragment().unwrap())).unwrap();
    let values: std::collections::HashMap<_, _> = fragment.query_pairs().into_owned().collect();
    json!({"classroom_id": CLASSROOM_ID, "protocol_version": 1,
        "client_version": "0.3.99", "required_capabilities": ["student-invite-v1"],
        "invite_token": values["invite"], "username": invite["username"], "password": "student-password-123"})
}

#[tokio::test]
async fn classroom_disabled_by_default_and_discovery_is_minimal() {
    let (app, state) = fixture(false);
    assert_eq!(
        request(
            &app,
            "GET",
            "/.well-known/cyanrex-classroom",
            None,
            "",
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let teacher = cookie(&state, "admin").await;
    assert_eq!(
        request(
            &app,
            "POST",
            "/classroom/invitations",
            Some(&teacher),
            ORIGIN,
            json!({"username":"student"})
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let (app, _) = fixture(true);
    let (status, body) = request(
        &app,
        "GET",
        "/.well-known/cyanrex-classroom",
        None,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["classroom_id"], CLASSROOM_ID);
    assert_eq!(body["protocol_min"], 1);
    assert_eq!(body["protocol_max"], 1);
    assert_eq!(body["join_url"], format!("{ORIGIN}/join"));
    assert_eq!(body["product_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(body.as_object().unwrap().len(), 8);
}

#[tokio::test]
async fn classroom_management_is_teacher_only_and_csrf_protected() {
    let (app, state) = fixture(true);
    let student = cookie(&state, "existing-student").await;
    let teacher = cookie(&state, "teacher").await;
    for path in ["/classroom/invitations", "/classroom/invitations/revoke"] {
        for (identity, expected) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some(student.as_str()), StatusCode::FORBIDDEN),
        ] {
            assert_eq!(
                request(&app, "POST", path, identity, ORIGIN, json!({}))
                    .await
                    .0,
                expected
            );
        }
        for origin in ["https://evil.example", ""] {
            assert_eq!(
                request(&app, "POST", path, Some(&teacher), origin, json!({}))
                    .await
                    .0,
                StatusCode::FORBIDDEN
            );
        }
    }
    for username in ["ADMIN", " teacher ", "legacy-admin"] {
        assert_eq!(
            request(
                &app,
                "POST",
                "/classroom/invitations",
                Some(&teacher),
                ORIGIN,
                json!({"username":username})
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        request(
            &app,
            "POST",
            "/classroom/invitations",
            Some(&teacher),
            ORIGIN,
            json!({"username":"student", "role":"teacher"})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn classroom_invitation_joins_student_without_open_registration_or_automatic_login() {
    let (app, state) = fixture(true);
    let teacher = cookie(&state, "admin").await;
    let issued = invite(&app, &teacher, "invited-student").await;
    let body = join_body(&issued);
    let (status, joined) =
        request(&app, "POST", "/classroom/join", None, ORIGIN, body.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{joined}");
    assert_eq!(joined["account_name"], "invited-student");
    assert!(joined["secret"].as_str().unwrap().len() >= 16);
    assert!(state
        .auth_service
        .login("invited-student", "student-password-123", "invalid")
        .await
        .is_err());
    let otp = state
        .auth_service
        .generate_current_totp_for_user("invited-student")
        .unwrap();
    assert!(state
        .auth_service
        .login("invited-student", "student-password-123", &otp)
        .await
        .is_ok());
    assert!(!state.auth_service.is_teacher_username("invited-student"));
    assert_eq!(
        request(&app, "POST", "/classroom/join", None, ORIGIN, body)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn classroom_wrong_identity_protocol_capability_origin_or_username_cannot_redeem() {
    let (app, state) = fixture(true);
    let teacher = cookie(&state, "admin").await;
    let issued = invite(&app, &teacher, "bound-student").await;
    let body = join_body(&issued);
    for (field, value, expected) in [
        (
            "classroom_id",
            json!("different-teacher"),
            StatusCode::CONFLICT,
        ),
        ("protocol_version", json!(2), StatusCode::UPGRADE_REQUIRED),
        (
            "required_capabilities",
            json!(["unknown-required-feature"]),
            StatusCode::UPGRADE_REQUIRED,
        ),
        ("username", json!("someone-else"), StatusCode::FORBIDDEN),
        ("username", json!("teacher"), StatusCode::FORBIDDEN),
        ("invite_token", json!("0".repeat(64)), StatusCode::FORBIDDEN),
    ] {
        let mut invalid = body.clone();
        invalid[field] = value;
        assert_eq!(
            request(&app, "POST", "/classroom/join", None, ORIGIN, invalid)
                .await
                .0,
            expected,
            "{field}"
        );
    }
    for origin in ["https://evil.example", ""] {
        assert_eq!(
            request(&app, "POST", "/classroom/join", None, origin, body.clone())
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        request(&app, "POST", "/classroom/join", None, ORIGIN, body)
            .await
            .0,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn classroom_revocation_and_inventory_do_not_leak_invitation_secrets() {
    let (app, state) = fixture(true);
    let teacher = cookie(&state, "admin").await;
    let issued = invite(&app, &teacher, "revoked-student").await;
    let (status, inventory) = request(
        &app,
        "GET",
        "/classroom/invitations",
        Some(&teacher),
        ORIGIN,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        inventory["invitations"][0]["invite_id"],
        issued["invite_id"]
    );
    assert!(!inventory
        .to_string()
        .contains(join_body(&issued)["invite_token"].as_str().unwrap()));
    assert!(inventory["invitations"][0]["join_url"].is_null());
    assert_eq!(
        request(
            &app,
            "POST",
            "/classroom/invitations/revoke",
            Some(&teacher),
            ORIGIN,
            json!({"invite_id":issued["invite_id"].as_str().unwrap().to_uppercase()})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/classroom/join",
            None,
            ORIGIN,
            join_body(&issued)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn classroom_concurrent_redemption_is_once_and_oversized_requests_are_rejected() {
    let (app, state) = fixture(true);
    let teacher = cookie(&state, "admin").await;
    let issued = invite(&app, &teacher, "racing-student").await;
    let body = join_body(&issued);
    let (first, second) = tokio::join!(
        request(&app, "POST", "/classroom/join", None, ORIGIN, body.clone()),
        request(&app, "POST", "/classroom/join", None, ORIGIN, body)
    );
    let mut statuses = [first.0.as_u16(), second.0.as_u16()];
    statuses.sort();
    assert_eq!(statuses, [201, 403]);
    assert_eq!(
        request(
            &app,
            "POST",
            "/classroom/join",
            None,
            ORIGIN,
            json!({"password":"x".repeat(5000)})
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn classroom_disabled_join_does_not_reveal_reserved_teacher_names() {
    let (app, _) = fixture(false);
    for username in ["admin", "teacher", "unlisted-student"] {
        let (status, _) = request(
            &app,
            "POST",
            "/classroom/join",
            None,
            ORIGIN,
            json!({"classroom_id":CLASSROOM_ID,"protocol_version":1,"client_version":"0.3.6",
                "required_capabilities":["student-invite-v1"],"invite_token":"a".repeat(64),
                "username":username,"password":"student-password-123"}),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }
}

#[tokio::test]
async fn classroom_existing_account_is_not_reset_and_failed_creation_consumes_invitation() {
    let (app, state) = fixture(true);
    state
        .auth_service
        .register("existing-student", "original-password-123")
        .await
        .unwrap();
    let teacher = cookie(&state, "admin").await;
    let issued = invite(&app, &teacher, "existing-student").await;
    let body = join_body(&issued);
    let mut elevated = body.clone();
    elevated["role"] = json!("teacher");
    assert_eq!(
        request(&app, "POST", "/classroom/join", None, ORIGIN, elevated)
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, response) =
        request(&app, "POST", "/classroom/join", None, ORIGIN, body.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(response["secret"].is_null());
    assert_eq!(
        request(&app, "POST", "/classroom/join", None, ORIGIN, body)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let otp = state
        .auth_service
        .generate_current_totp_for_user("existing-student")
        .unwrap();
    assert!(state
        .auth_service
        .login("existing-student", "original-password-123", &otp)
        .await
        .is_ok());
}
