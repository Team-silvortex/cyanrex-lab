use super::test_support;
use super::{
    test_support::*, Backend as CreateBackend, TaskContentHttpConfigError, TaskCreateHttpState,
};
use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode},
};
use serde_json::json;

#[test]
fn create_configuration_validates_origin_then_capacity() {
    for capacity in [1, 4, 16] {
        assert!(TaskCreateHttpState::configured(
            CreateBackend::Test(Backend::new(Outcome::Success)),
            ORIGIN,
            capacity
        )
        .is_ok());
    }
    for capacity in [0, 17, usize::MAX] {
        assert!(matches!(
            TaskCreateHttpState::configured(
                CreateBackend::Test(Backend::new(Outcome::Success)),
                ORIGIN,
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidCapacity)
        ));
        assert!(matches!(
            TaskCreateHttpState::configured(
                CreateBackend::Test(Backend::new(Outcome::Success)),
                "http://platform.example",
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidOrigin)
        ));
    }
}

#[tokio::test]
async fn create_empty_task_projects_only_committed_snapshot_without_code_or_run() {
    let backend = test_support::Backend::new(Outcome::Success);
    let response = capture(app(backend.clone(), 4), request("POST", CREATE, body())).await;
    assert_eq!(response.status, StatusCode::CREATED);
    private(&response);
    assert_eq!(response.json["schema_version"], 1);
    assert_eq!(response.json["task"]["title"], "A task without code");
    assert_eq!(response.json["task"]["status"], "draft");
    assert_eq!(response.json["task"]["revision"], 1);
    assert_eq!(response.json["task"]["reference"]["task_id"], ID);
    assert_eq!(response.json["task"]["input_refs"], json!([]));
    assert_eq!(
        response.json["manifest"],
        json!({"schema_version":1,"title":"A task without code","payload":[]})
    );
    assert_eq!(response.json.as_object().unwrap().len(), 3);
    assert!(response.json.get("contents").is_none());
    assert_eq!(backend.calls(), 1);
    let received = backend.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].token, TOKEN);
    assert_eq!(received[0].id, ID.parse().unwrap());
    assert_eq!(received[0].title, "A task without code");
}

#[tokio::test]
async fn create_route_is_exact_post_only_without_query_or_other_capabilities() {
    let backend = test_support::Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for path in [
        "/",
        "/platform/v1/tasks/",
        "/platform/v1/task-index",
        "/platform/v1/catalog-tasks",
        "/platform/v1/text-artifacts",
        "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136",
        "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136/content",
    ] {
        error(
            &capture(app.clone(), request("POST", path, untouched())).await,
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        );
    }
    for method in ["GET", "PUT", "PATCH", "DELETE", "OPTIONS"] {
        let response = capture(app.clone(), request(method, CREATE, untouched())).await;
        error(
            &response,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "not_attempted",
        );
        assert_eq!(response.headers[header::ALLOW], "POST");
    }
    let head = capture(app.clone(), request("HEAD", CREATE, untouched())).await;
    assert_eq!(head.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(head.headers[header::ALLOW], "POST");
    assert_eq!(head.headers[header::CONTENT_LENGTH], "0");
    assert!(head.json.is_null());
    private(&head);
    for path in [
        "/platform/v1/tasks?",
        "/platform/v1/tasks?owner=other",
        "/platform/v1/tasks?token=private",
    ] {
        error(
            &capture(app.clone(), request("POST", path, untouched())).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn create_rejects_missing_duplicate_or_foreign_authority_headers_before_body() {
    let backend = test_support::Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for name in ["origin", "x-cyanrex-platform-request"] {
        for duplicate in [false, true] {
            let mut req = request("POST", CREATE, untouched());
            if duplicate {
                req.headers_mut()
                    .append(name, HeaderValue::from_static("1"));
            } else {
                req.headers_mut().remove(name);
            }
            error(
                &capture(app.clone(), req).await,
                StatusCode::FORBIDDEN,
                "request_forbidden",
                "not_attempted",
            );
        }
    }
    let mut req = request("POST", CREATE, untouched());
    req.headers_mut().insert(
        header::ORIGIN,
        HeaderValue::from_static("https://foreign.example"),
    );
    error(
        &capture(app.clone(), req).await,
        StatusCode::FORBIDDEN,
        "request_forbidden",
        "not_attempted",
    );
    for cookie in [None, Some("cyanrex_session=synthetic"), Some("cyanrex_platform_session=not-a-token"), Some("cyanrex_platform_session=ae0b35cd-c9b8-413c-a892-854b15959f41; cyanrex_platform_session=ae0b35cd-c9b8-413c-a892-854b15959f41")] {
        let mut req = request("POST", CREATE, untouched()); req.headers_mut().remove(header::COOKIE);
        if let Some(cookie) = cookie { req.headers_mut().insert(header::COOKIE, cookie.parse().unwrap()); }
        error(&capture(app.clone(), req).await, StatusCode::UNAUTHORIZED, "invalid_session", "not_attempted");
    }
    let mut req = request("POST", CREATE, untouched());
    req.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer ignored"),
    );
    error(
        &capture(app, req).await,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn create_wire_record_rejects_aliases_duplicates_versions_and_authority_fields() {
    let backend = test_support::Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    let valid = json!({"schema_version":1,"task_id":ID,"title":"Allowed"});
    let mut invalid = vec!["null".into(), "[]".into(), format!("[1,{ID:?},\"Allowed\"]"), "{}".into(),
        format!("{{\"schema_version\":1,\"schema_version\":1,\"task_id\":{ID:?},\"title\":\"Allowed\"}}"),
        format!("{{\"schema_version\":1,\"task_id\":{ID:?},\"task_id\":{ID:?},\"title\":\"Allowed\"}}"),
        format!("{{\"schema_version\":1,\"task_id\":{ID:?},\"title\":\"Allowed\",\"title\":\"Allowed\"}}")];
    for (field, value) in [
        ("schema_version", json!(2)),
        ("schema_version", json!("1")),
        ("schema_version", json!(1.0)),
        ("task_id", json!("00000000-0000-0000-0000-000000000000")),
        ("task_id", json!(ID.to_uppercase())),
        ("title", json!(null)),
        ("title", json!("")),
        ("title", json!("   ")),
        ("title", json!("bad\nname")),
        ("title", json!("x".repeat(257))),
        ("title", json!("é".repeat(129))),
    ] {
        let mut changed = valid.clone();
        changed[field] = value;
        invalid.push(changed.to_string());
    }
    for field in [
        "owner",
        "scope",
        "namespace",
        "status",
        "revision",
        "manifest",
        "payload",
        "definition",
        "contents",
        "run",
    ] {
        let mut changed = valid.clone();
        changed[field] = json!(null);
        invalid.push(changed.to_string());
    }
    for raw in invalid {
        error(
            &capture(app.clone(), request("POST", CREATE, Body::from(raw))).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
    let valid = json!({"schema_version":1,"task_id":ID,"title":"é".repeat(128)}).to_string();
    let response = capture(app, request("POST", CREATE, Body::from(valid))).await;
    assert_eq!(response.status, StatusCode::CREATED);
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn create_body_policy_rejects_encoding_type_stream_overflow_and_transport_error() {
    let backend = test_support::Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    let mut req = request("POST", CREATE, untouched());
    req.headers_mut().insert(
        header::CONTENT_ENCODING,
        HeaderValue::from_static("identity"),
    );
    error(
        &capture(app.clone(), req).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    for content_type in [
        None,
        Some("text/plain"),
        Some("application/json;charset=utf-8"),
    ] {
        let mut req = request("POST", CREATE, untouched());
        req.headers_mut().remove(header::CONTENT_TYPE);
        if let Some(value) = content_type {
            req.headers_mut()
                .insert(header::CONTENT_TYPE, value.parse().unwrap());
        }
        error(
            &capture(app.clone(), req).await,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "not_attempted",
        );
    }
    let mut req = request("POST", CREATE, Body::from(vec![b' '; 64 * 1024 + 1]));
    req.headers_mut()
        .insert(header::CONTENT_LENGTH, HeaderValue::from_static("1"));
    error(
        &capture(app.clone(), req).await,
        StatusCode::PAYLOAD_TOO_LARGE,
        "request_too_large",
        "not_attempted",
    );
    let failed = Body::from_stream(futures_util::stream::iter([Err::<axum::body::Bytes, _>(
        std::io::Error::other("synthetic body error"),
    )]));
    error(
        &capture(app, request("POST", CREATE, failed)).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn create_dispatched_errors_remain_unconfirmed_without_cookie_retry_or_diagnostics() {
    for (outcome, status, code) in [
        (
            Outcome::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
        (
            Outcome::InvalidSession,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
        ),
        (Outcome::Conflict, StatusCode::CONFLICT, "conflict"),
        (Outcome::Denied, StatusCode::FORBIDDEN, "access_denied"),
    ] {
        let backend = test_support::Backend::new(outcome);
        error(
            &capture(app(backend.clone(), 4), request("POST", CREATE, body())).await,
            status,
            code,
            "unconfirmed",
        );
        assert_eq!(backend.calls(), 1);
    }
}
