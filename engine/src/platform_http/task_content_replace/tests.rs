use super::{
    test_support::*, Backend as ReplaceBackend, TaskContentHttpConfigError,
    TaskContentReplaceHttpState,
};
use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode},
};
use serde_json::json;

#[test]
fn replace_configuration_validates_origin_then_capacity() {
    for capacity in [1, 4, 16] {
        assert!(TaskContentReplaceHttpState::configured(
            ReplaceBackend::Test(Backend::new(Outcome::Success)),
            ORIGIN,
            capacity
        )
        .is_ok());
    }
    for capacity in [0, 17, usize::MAX] {
        assert!(matches!(
            TaskContentReplaceHttpState::configured(
                ReplaceBackend::Test(Backend::new(Outcome::Success)),
                ORIGIN,
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidCapacity)
        ));
        assert!(matches!(
            TaskContentReplaceHttpState::configured(
                ReplaceBackend::Test(Backend::new(Outcome::Success)),
                "http://platform.example",
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidOrigin)
        ));
    }
}

#[tokio::test]
async fn replace_projects_only_committed_snapshot_without_contents_code_or_run() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    let response = capture(app.clone(), request("PUT", REPLACE, body())).await;
    assert_eq!(response.status, StatusCode::OK);
    private(&response);
    assert_eq!(response.json["schema_version"], 1);
    assert_eq!(response.json["task"]["title"], "Renamed without code");
    assert_eq!(response.json["task"]["status"], "draft");
    assert_eq!(response.json["task"]["revision"], 2);
    assert_eq!(response.json["task"]["reference"]["task_id"], ID);
    assert_eq!(response.json["task"]["input_refs"], json!([]));
    assert_eq!(response.json["manifest"], wire()["manifest"]);
    assert_eq!(response.json.as_object().unwrap().len(), 3);
    assert!(response.json.get("contents").is_none());
    assert_eq!(backend.calls(), 1);
    let received = backend.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].token, TOKEN);
    assert_eq!(received[0].id, ID.parse().unwrap());
    assert_eq!(u64::from(received[0].revision), 1);
    assert_eq!(received[0].manifest.title(), "Renamed without code");
    assert!(received[0].manifest.payload().is_empty());
    // Transport accepts exact inert references, not inline bytes or evidence of ownership.
    let artifact = json!({"workspace":{"authority_id":"00000000-0000-0000-0000-000000001069",
        "workspace_id":"00000000-0000-0000-0000-00000000106a"},
        "artifact_id":"00000000-0000-4000-8000-000000000003",
        "revision_id":"00000000-0000-4000-8000-000000000004","sha256":"a".repeat(64)});
    let mut nonempty = wire();
    nonempty["manifest"]["title"] = json!("Non-code notes");
    nonempty["manifest"]["payload"] = json!([{"kind":"text","filename":"notes.md",
        "language":"plaintext","artifact":artifact}]);
    let response = capture(
        app,
        request("PUT", REPLACE, Body::from(nonempty.to_string())),
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json["manifest"], nonempty["manifest"]);
    assert_eq!(response.json["task"]["input_refs"], json!([artifact]));
    assert!(response.json.get("contents").is_none());
    assert_eq!(backend.calls(), 2);
}

#[tokio::test]
async fn replace_route_is_raw_canonical_put_only_without_query_or_other_capabilities() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for path in [
        "/",
        "/platform/v1/tasks",
        "/platform/v1/tasks/",
        "/platform/v1/task-index",
        "/platform/v1/catalog-tasks",
        "/platform/v1/text-artifacts",
        "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136",
        "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136/status",
        "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136/content/",
        "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136/content/extra",
    ] {
        error(
            &capture(app.clone(), request("PUT", path, untouched())).await,
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        );
    }
    for id in [
        "not-a-uuid",
        "00000000-0000-0000-0000-000000000000",
        "E3698554-8EB3-4B6D-884E-8074E1555136",
        "e36985548eb34b6d884e8074e1555136",
        "%653698554-8eb3-4b6d-884e-8074e1555136",
    ] {
        error(
            &capture(
                app.clone(),
                request(
                    "PUT",
                    &format!("/platform/v1/tasks/{id}/content"),
                    untouched(),
                ),
            )
            .await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    for method in ["GET", "POST", "PATCH", "DELETE", "OPTIONS"] {
        let response = capture(app.clone(), request(method, REPLACE, untouched())).await;
        error(
            &response,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "not_attempted",
        );
        assert_eq!(response.headers[header::ALLOW], "PUT");
    }
    let head = capture(app.clone(), request("HEAD", REPLACE, untouched())).await;
    assert_eq!(head.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(head.headers[header::ALLOW], "PUT");
    assert_eq!(head.headers[header::CONTENT_LENGTH], "0");
    assert!(head.json.is_null());
    private(&head);
    for query in ["?", "?owner=other", "?token=private", "?revision=2"] {
        error(
            &capture(
                app.clone(),
                request("PUT", &format!("{REPLACE}{query}"), untouched()),
            )
            .await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn replace_rejects_missing_duplicate_or_foreign_authority_headers_before_body() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for name in ["origin", "x-cyanrex-platform-request"] {
        for duplicate in [false, true] {
            let mut req = request("PUT", REPLACE, untouched());
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
    let mut req = request("PUT", REPLACE, untouched());
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
        let mut req = request("PUT", REPLACE, untouched()); req.headers_mut().remove(header::COOKIE);
        if let Some(cookie) = cookie { req.headers_mut().insert(header::COOKIE, cookie.parse().unwrap()); }
        error(&capture(app.clone(), req).await, StatusCode::UNAUTHORIZED, "invalid_session", "not_attempted");
    }
    let mut req = request("PUT", REPLACE, untouched());
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
async fn replace_wire_record_and_nested_manifest_reject_aliases_duplicates_and_authority_fields() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    let valid = wire();
    let mut invalid = vec!["null".into(), "[]".into(), "[1,1,{}]".into(), "{}".into(),
        "{\"schema_version\":1,\"schema_version\":1,\"expected_revision\":1,\"manifest\":{\"schema_version\":1,\"title\":\"Valid\",\"payload\":[]}}".into(),
        "{\"schema_version\":1,\"expected_revision\":1,\"expected_revision\":1,\"manifest\":{\"schema_version\":1,\"title\":\"Valid\",\"payload\":[]}}".into(),
        "{\"schema_version\":1,\"expected_revision\":1,\"manifest\":{\"schema_version\":1,\"title\":\"Valid\",\"title\":\"Valid\",\"payload\":[]}}".into()];
    for (field, value) in [
        ("schema_version", json!(2)),
        ("schema_version", json!("1")),
        ("schema_version", json!(1.0)),
        ("expected_revision", json!(0)),
        ("expected_revision", json!(-1)),
        ("expected_revision", json!(1.5)),
        ("expected_revision", json!("1")),
        ("expected_revision", json!(9_007_199_254_740_992u64)),
        ("manifest", json!(null)),
        ("manifest", json!([1, "Valid", []])),
    ] {
        let mut changed = valid.clone();
        changed[field] = value;
        invalid.push(changed.to_string());
    }
    for (field, value) in [
        ("schema_version", json!(2)),
        ("title", json!("")),
        ("title", json!("bad\nname")),
        ("title", json!("é".repeat(129))),
        ("payload", json!([null])),
        ("owner", json!(null)),
    ] {
        let mut changed = valid.clone();
        changed["manifest"][field] = value;
        invalid.push(changed.to_string());
    }
    for field in [
        "owner",
        "scope",
        "namespace",
        "task_id",
        "status",
        "revision",
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
            &capture(app.clone(), request("PUT", REPLACE, Body::from(raw))).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
    let mut boundary = valid;
    boundary["manifest"]["title"] = json!("é".repeat(128));
    assert_eq!(
        capture(
            app,
            request("PUT", REPLACE, Body::from(boundary.to_string()))
        )
        .await
        .status,
        StatusCode::OK
    );
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn replace_body_policy_rejects_encoding_type_stream_overflow_and_transport_error() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    let mut req = request("PUT", REPLACE, untouched());
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
        let mut req = request("PUT", REPLACE, untouched());
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
    let mut req = request("PUT", REPLACE, Body::from(vec![b' '; 64 * 1024 + 1]));
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
        &capture(app, request("PUT", REPLACE, failed)).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn replace_dispatched_errors_stay_unconfirmed_without_retry_cookie_or_diagnostics() {
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
        (Outcome::Denied, StatusCode::FORBIDDEN, "access_denied"),
        (Outcome::Missing, StatusCode::NOT_FOUND, "not_found"),
        (Outcome::Stale, StatusCode::CONFLICT, "stale_revision"),
        (
            Outcome::NonDraft,
            StatusCode::CONFLICT,
            "invalid_transition",
        ),
        (Outcome::Noop, StatusCode::BAD_REQUEST, "invalid_request"),
    ] {
        let backend = Backend::new(outcome);
        error(
            &capture(app(backend.clone(), 4), request("PUT", REPLACE, body())).await,
            status,
            code,
            "unconfirmed",
        );
        assert_eq!(backend.calls(), 1);
    }
}
