use super::{test_support::*, Backend as PublishBackend, TaskTextArtifactPublishHttpState};
use crate::platform_http::TaskContentHttpConfigError;
use axum::http::{header, HeaderValue, StatusCode};
use serde_json::json;

#[test]
fn task_text_publish_configuration_checks_origin_capacity_and_keeps_trusted_scope() {
    for capacity in [1, 4, 16] {
        assert!(TaskTextArtifactPublishHttpState::configured(
            PublishBackend::Test(Backend::new(Outcome::Success)),
            ORIGIN,
            scope(),
            capacity
        )
        .is_ok());
    }
    for capacity in [0, 17, usize::MAX] {
        assert!(matches!(
            TaskTextArtifactPublishHttpState::configured(
                PublishBackend::Test(Backend::new(Outcome::Success)),
                ORIGIN,
                scope(),
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidCapacity)
        ));
        assert!(matches!(
            TaskTextArtifactPublishHttpState::configured(
                PublishBackend::Test(Backend::new(Outcome::Success)),
                "http://platform.example",
                scope(),
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidOrigin)
        ));
    }
}

#[tokio::test]
async fn task_text_publish_forwards_complete_baseline_and_returns_only_synthetic_revision() {
    let backend = Backend::new(Outcome::Success);
    let response = capture(app(backend.clone(), 4), request("POST", CREATE, body())).await;
    assert_eq!(response.status, StatusCode::CREATED);
    private(&response);
    assert_eq!(
        response.json,
        json!({"schema_version":1,"artifact":published(&baseline(), ID.parse().unwrap(), REVISION.parse().unwrap(), TEXT)})
    );
    assert!(response.json.get("text").is_none());
    assert!(response.json.get("task").is_none());
    assert_eq!(backend.calls(), 1);
    let received = backend.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].token, TOKEN);
    assert_eq!(received[0].expected, baseline());
    assert_eq!(received[0].id, ID.parse().unwrap());
    assert_eq!(received[0].revision, REVISION.parse().unwrap());
    assert_eq!(received[0].text, TEXT);
}

#[tokio::test]
async fn task_text_publish_is_exact_post_only_without_other_route_or_query_capabilities() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for path in [
        "/",
        "/platform/v1/text-artifacts",
        "/platform/v1/tasks",
        "/platform/v1/task-index",
        "/platform/v1/session/login",
        "/platform/v1/file-chunks",
        &format!("{CREATE}/"),
        &format!("{CREATE}/extra"),
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
    private(&head);
    assert_eq!(head.headers[header::ALLOW], "POST");
    assert_eq!(head.headers[header::CONTENT_LENGTH], "0");
    assert!(head.json.is_null());
    for query in ["?", "?owner=other", "?token=private", "?revision=7"] {
        error(
            &capture(
                app.clone(),
                request("POST", &format!("{CREATE}{query}"), untouched()),
            )
            .await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    for id in [
        "not-a-task".into(),
        TASK.to_uppercase(),
        TASK.replace('-', ""),
        TASK.replace('-', "%2d"),
    ] {
        error(
            &capture(
                app.clone(),
                request("POST", &CREATE.replace(TASK, &id), untouched()),
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
async fn task_text_publish_ingress_headers_reject_before_body_poll() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for name in ["origin", "x-cyanrex-platform-request"] {
        for duplicate in [false, true] {
            let mut req = request("POST", CREATE, untouched());
            if duplicate {
                req.headers_mut().append(
                    name,
                    HeaderValue::from_static(if name == "origin" { ORIGIN } else { "1" }),
                );
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
    for (name, value) in [
        ("origin", "https://foreign.example"),
        ("x-cyanrex-platform-request", "true"),
    ] {
        let mut req = request("POST", CREATE, untouched());
        req.headers_mut().insert(name, value.parse().unwrap());
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn task_text_publish_dedicated_cookie_has_no_bearer_or_teaching_fallback() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone(), 4);
    for cookie in [
        None,
        Some("cyanrex_session=synthetic".into()),
        Some("cyanrex_platform_session=not-a-token".into()),
        Some(format!(
            "cyanrex_platform_session={TOKEN}; cyanrex_platform_session={TOKEN}"
        )),
        Some(format!("cyanrex_platform_session={}", TOKEN.to_uppercase())),
    ] {
        let mut req = request("POST", CREATE, untouched());
        req.headers_mut().remove(header::COOKIE);
        if let Some(cookie) = cookie {
            req.headers_mut()
                .insert(header::COOKIE, cookie.parse().unwrap());
        }
        error(
            &capture(app.clone(), req).await,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
            "not_attempted",
        );
    }
    for authorization in ["", "Bearer ignored"] {
        let mut req = request("POST", CREATE, untouched());
        req.headers_mut()
            .insert(header::AUTHORIZATION, authorization.parse().unwrap());
        error(
            &capture(app.clone(), req).await,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
            "not_attempted",
        );
    }
    let mut duplicate = request("POST", CREATE, untouched());
    duplicate.headers_mut().append(
        header::COOKIE,
        format!("cyanrex_platform_session={TOKEN}").parse().unwrap(),
    );
    error(
        &capture(app.clone(), duplicate).await,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "not_attempted",
    );
    let mut huge = request("POST", CREATE, untouched());
    huge.headers_mut().append(
        header::COOKIE,
        format!("unrelated={}", "x".repeat(8193)).parse().unwrap(),
    );
    error(
        &capture(app, huge).await,
        StatusCode::PAYLOAD_TOO_LARGE,
        "request_too_large",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn task_text_publish_dispatch_errors_are_private_unconfirmed_including_artifact_conflict() {
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
        (Outcome::ArtifactMissing, StatusCode::NOT_FOUND, "not_found"),
        (Outcome::Stale, StatusCode::CONFLICT, "stale_revision"),
        (
            Outcome::NonDraft,
            StatusCode::CONFLICT,
            "invalid_transition",
        ),
        (Outcome::ArtifactConflict, StatusCode::CONFLICT, "conflict"),
        (
            Outcome::ArtifactStale,
            StatusCode::CONFLICT,
            "stale_revision",
        ),
        (
            Outcome::InvalidInput,
            StatusCode::BAD_REQUEST,
            "invalid_request",
        ),
        (
            Outcome::ArtifactInvalid,
            StatusCode::BAD_REQUEST,
            "invalid_request",
        ),
        (
            Outcome::Corrupt,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
        (
            Outcome::Unsupported,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
        (
            Outcome::InvalidNamespace,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
    ] {
        let backend = Backend::new(outcome);
        error(
            &capture(app(backend.clone(), 4), request("POST", CREATE, body())).await,
            status,
            code,
            "unconfirmed",
        );
        assert_eq!(backend.calls(), 1);
    }
}
