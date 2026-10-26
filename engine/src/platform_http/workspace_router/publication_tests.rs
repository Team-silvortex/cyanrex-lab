//! Synthetic route/budget evidence, never persistence, shared original storage or host acceptance.
use super::{build_platform_workspace_router, test_support::*};
use crate::platform_http::{config, TaskContentHttpConfigError};
use axum::{
    body::{Body, HttpBody},
    http::{header, HeaderValue, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[test]
fn publication_configuration_checks_each_of_seven_capacities_after_origin() {
    for slot in 0..7 {
        for capacity in [1, 4, 16] {
            let mut capacities = [4; 7];
            capacities[slot] = capacity;
            assert_eq!(config::validate(ORIGIN, &capacities), Ok(()));
        }
        for invalid in [0, 17, usize::MAX] {
            let mut capacities = [4; 7];
            capacities[slot] = invalid;
            assert_eq!(
                config::validate(ORIGIN, &capacities),
                Err(TaskContentHttpConfigError::InvalidCapacity)
            );
            assert_eq!(
                config::validate("http://platform.example", &capacities),
                Err(TaskContentHttpConfigError::InvalidOrigin)
            );
        }
    }
}

#[tokio::test]
async fn publication_workspace_adds_only_explicit_text_post_and_preserves_all_old_commands() {
    let login = LoginBackend::new(LoginOutcome::Success);
    let publisher = PublicationBackend::new(PublicationOutcome::Success);
    let app = build_platform_workspace_router(
        publication_state(login.clone(), publisher.clone(), [4; 7]).await,
    );
    let response = capture(app.clone(), request("POST", PUBLISH, publication_body())).await;
    assert_eq!(response.status, StatusCode::CREATED);
    private(&response);
    assert!(!response.headers.contains_key(header::SET_COOKIE));
    assert_eq!(response.json["schema_version"], 1);
    assert_eq!(response.json["artifact"]["kind"], "cyanrex.task-text");
    assert_eq!(response.json["artifact"]["media_type"], "text/plain");
    assert_eq!(response.json["artifact"]["sequence"], 1);
    assert!(response.json["artifact"]["parent"].is_null());
    assert_eq!(response.json.as_object().unwrap().len(), 2);
    assert!(response.json.get("text").is_none());
    assert!(response.json.get("task").is_none());
    assert_eq!(publisher.calls(), 1);
    issued(&capture(app.clone(), request("POST", LOGIN, login_body())).await);
    dispatched(&capture(app.clone(), request("POST", CREATE, create_body())).await);
    dispatched(&capture(app.clone(), request("PUT", REPLACE, replace_body())).await);
    for (method, path) in [
        ("GET", SESSION),
        ("POST", LOGOUT),
        ("GET", INDEX),
        ("GET", CONTENT),
    ] {
        dispatched(&capture(app.clone(), request(method, path, Body::empty())).await);
    }
    assert_eq!(login.calls(), 1);
    assert_eq!(
        publisher.calls(),
        1,
        "old commands must never invoke text publication"
    );
    for (method, path) in [
        ("GET", "/"),
        ("POST", "/auth/login"),
        ("POST", "/ebpf/run"),
        ("POST", "/scripts/save"),
        ("GET", "/modules"),
        ("GET", "/events"),
        ("POST", "/platform/v1/catalog-tasks"),
        ("GET", "/platform/v1/file-manifests"),
        ("GET", "/platform/v1/file-chunks"),
        ("POST", "/platform/v1/text-artifacts/"),
        (
            "PUT",
            "/platform/v1/tasks/e3698554-8eb3-4b6d-884e-8074e1555136/status",
        ),
    ] {
        error(
            &capture(app.clone(), request(method, path, untouched())).await,
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        );
    }
    // The old pooled text router's exact-read edge must not be merged as part of this opt-in.
    let read = format!("{PUBLISH}/e3698554-8eb3-4b6d-884e-8074e1555136/revisions/2de3dd0d-46b0-4140-a3d2-9697b2e79578/{}", "a".repeat(64));
    error(
        &capture(app, request("GET", &read, untouched())).await,
        StatusCode::NOT_FOUND,
        "not_found",
        "not_attempted",
    );
    assert_eq!(publisher.calls(), 1);
}

#[tokio::test]
async fn legacy_workspace_composition_keeps_text_publication_absent_at_all_configured_capacities() {
    let login = LoginBackend::new(LoginOutcome::Success);
    for capacity in [1, 4, 16] {
        let state = state(login.clone(), [capacity; 6]).await;
        assert!(state.text.is_none());
        let app = build_platform_workspace_router(state);
        error(
            &capture(app.clone(), request("POST", PUBLISH, untouched())).await,
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        );
        issued(&capture(app, request("POST", LOGIN, login_body())).await);
    }
    assert_eq!(login.calls(), 3);
}

#[tokio::test]
async fn publication_workspace_shares_clones_but_keeps_all_seven_response_lanes_independent() {
    let login = LoginBackend::new(LoginOutcome::Success);
    let publisher = PublicationBackend::new(PublicationOutcome::Success);
    let state = publication_state(login.clone(), publisher.clone(), [1; 7]).await;
    let first = build_platform_workspace_router(state.clone());
    let second = build_platform_workspace_router(state);
    let published = first
        .clone()
        .oneshot(request("POST", PUBLISH, publication_body()))
        .await
        .unwrap();
    assert_eq!(published.status(), StatusCode::CREATED);
    let mut held = vec![];
    for (method, path, body, status) in [
        ("POST", LOGIN, login_body(), StatusCode::OK),
        (
            "GET",
            SESSION,
            Body::empty(),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        ("GET", INDEX, Body::empty(), StatusCode::SERVICE_UNAVAILABLE),
        (
            "GET",
            CONTENT,
            Body::empty(),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            "POST",
            CREATE,
            create_body(),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            "PUT",
            REPLACE,
            replace_body(),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ] {
        let response = second
            .clone()
            .oneshot(request(method, path, body))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            status,
            "publication saturation must not block {path}"
        );
        held.push(response);
    }
    for (method, path) in [
        ("POST", PUBLISH),
        ("POST", LOGIN),
        ("GET", SESSION),
        ("POST", LOGOUT),
        ("GET", INDEX),
        ("GET", CONTENT),
        ("POST", CREATE),
        ("PUT", REPLACE),
    ] {
        error(
            &capture(first.clone(), request(method, path, untouched())).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        );
    }
    assert_eq!(publisher.calls(), 1);
    assert_eq!(login.calls(), 1);
    let mut body = published.into_body();
    assert!(!body.is_end_stream());
    let retained = body.frame().await.unwrap().unwrap().into_data().unwrap();
    assert!(!retained.is_empty());
    assert!(body.is_end_stream());
    let publication = capture(second.clone(), request("POST", PUBLISH, publication_body())).await;
    assert_eq!(publication.status, StatusCode::CREATED);
    assert_eq!(publisher.calls(), 2);
    for (method, path) in [
        ("POST", LOGIN),
        ("GET", SESSION),
        ("GET", INDEX),
        ("GET", CONTENT),
        ("POST", CREATE),
        ("PUT", REPLACE),
    ] {
        error(
            &capture(first.clone(), request(method, path, untouched())).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        );
    }
    let published = first
        .clone()
        .oneshot(request("POST", PUBLISH, publication_body()))
        .await
        .unwrap();
    assert_eq!(publisher.calls(), 3);
    drop(held.remove(2)); // Drop only the index response; publication is still held.
    dispatched(&capture(second.clone(), request("GET", INDEX, Body::empty())).await);
    error(
        &capture(second.clone(), request("POST", PUBLISH, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    drop(published);
    let publication = capture(second, request("POST", PUBLISH, publication_body())).await;
    assert_eq!(publication.status, StatusCode::CREATED);
    assert!(
        !retained.is_empty(),
        "retaining JSON bytes must not retain a response permit"
    );
    assert_eq!(publisher.calls(), 4);
}

#[tokio::test]
async fn separately_constructed_publication_workspace_states_have_additive_publication_budgets() {
    let login = LoginBackend::new(LoginOutcome::Success);
    let publisher = PublicationBackend::new(PublicationOutcome::Unavailable);
    let first = build_platform_workspace_router(
        publication_state(login.clone(), publisher.clone(), [1; 7]).await,
    );
    let second =
        build_platform_workspace_router(publication_state(login, publisher.clone(), [1; 7]).await);
    let held = first
        .clone()
        .oneshot(request("POST", PUBLISH, publication_body()))
        .await
        .unwrap();
    assert_eq!(held.status(), StatusCode::SERVICE_UNAVAILABLE);
    error(
        &capture(first.clone(), request("POST", PUBLISH, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    dispatched(&capture(second, request("POST", PUBLISH, publication_body())).await);
    assert_eq!(publisher.calls(), 2);
    drop(held);
    dispatched(&capture(first, request("POST", PUBLISH, publication_body())).await);
    assert_eq!(publisher.calls(), 3);
}

#[tokio::test]
async fn publication_workspace_preserves_strict_post_headers_query_and_no_cookie_projection() {
    let publisher = PublicationBackend::new(PublicationOutcome::Success);
    let app = build_platform_workspace_router(
        publication_state(
            LoginBackend::new(LoginOutcome::Success),
            publisher.clone(),
            [1; 7],
        )
        .await,
    );
    for method in ["GET", "PUT", "PATCH", "DELETE", "OPTIONS"] {
        let response = capture(app.clone(), request(method, PUBLISH, untouched())).await;
        error(
            &response,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "not_attempted",
        );
        assert_eq!(response.headers[header::ALLOW], "POST");
    }
    let head = capture(app.clone(), request("HEAD", PUBLISH, untouched())).await;
    assert_eq!(head.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(head.headers[header::ALLOW], "POST");
    assert_eq!(head.headers[header::CONTENT_LENGTH], "0");
    assert!(head.json.is_null());
    private(&head);
    assert!(!head.headers.contains_key(header::SET_COOKIE));
    for query in ["?", "?owner=other", "?token=private"] {
        error(
            &capture(
                app.clone(),
                request("POST", &format!("{PUBLISH}{query}"), untouched()),
            )
            .await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    for name in ["origin", "x-cyanrex-platform-request"] {
        let mut req = request("POST", PUBLISH, untouched());
        req.headers_mut().remove(name);
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
    }
    let mut req = request("POST", PUBLISH, untouched());
    req.headers_mut().remove(header::COOKIE);
    error(
        &capture(app.clone(), req).await,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "not_attempted",
    );
    let mut req = request("POST", PUBLISH, untouched());
    req.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer no-fallback"),
    );
    error(
        &capture(app.clone(), req).await,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "not_attempted",
    );
    error(
        &capture(app.clone(), request("POST", PUBLISH, Body::from("{}"))).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    assert_eq!(publisher.calls(), 0);
    let response = capture(app, request("POST", PUBLISH, publication_body())).await;
    assert_eq!(response.status, StatusCode::CREATED);
    private(&response);
    assert!(!response.headers.contains_key(header::SET_COOKIE));
    assert_eq!(publisher.calls(), 1);
}
