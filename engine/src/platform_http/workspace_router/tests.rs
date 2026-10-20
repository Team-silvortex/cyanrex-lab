use super::build_platform_workspace_router;
use super::test_support::*;
use crate::platform_http::{config, TaskContentHttpConfigError};
use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode},
};
use tower::ServiceExt;

#[test]
fn workspace_configuration_validates_all_six_budgets_after_origin() {
    for login in [1, 4, 16] {
        for lifecycle in [1, 4, 16] {
            for index in [1, 4, 16] {
                for content in [1, 4, 16] {
                    for create in [1, 4, 16] {
                        for replace in [1, 4, 16] {
                            assert_eq!(
                                config::validate(
                                    ORIGIN,
                                    &[login, lifecycle, index, content, create, replace]
                                ),
                                Ok(())
                            );
                        }
                    }
                }
            }
        }
    }
    for invalid in [0, 17, usize::MAX] {
        for capacities in [
            [invalid, 4, 4, 4, 4, 4],
            [4, invalid, 4, 4, 4, 4],
            [4, 4, invalid, 4, 4, 4],
            [4, 4, 4, invalid, 4, 4],
            [4, 4, 4, 4, invalid, 4],
            [4, 4, 4, 4, 4, invalid],
        ] {
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
async fn workspace_routes_add_only_private_empty_create_reads_and_manifest_replacement() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let app = app(backend.clone()).await;
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
    for path in [
        "/",
        "/auth/login",
        "/ebpf/run",
        "/scripts/save",
        "/modules",
        "/events",
        "/platform/v1/tasks/00000000-0000-4000-8000-000000000001/status",
        "/platform/v1/catalog-tasks",
        "/platform/v1/text-artifacts",
        "/platform/v1/task-index/",
        "/platform/v1/session/extra",
    ] {
        error(
            &capture(app.clone(), request("POST", path, untouched())).await,
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        );
    }
    for (path, method, allow) in [
        (LOGIN, "GET", "POST"),
        (SESSION, "POST", "GET"),
        (LOGOUT, "GET", "POST"),
        (INDEX, "POST", "GET"),
        (INDEX, "PUT", "GET"),
        (INDEX, "DELETE", "GET"),
        (INDEX, "OPTIONS", "GET"),
        (CONTENT, "POST", "GET"),
        (CONTENT, "PUT", "GET"),
        (CONTENT, "DELETE", "GET"),
        (CONTENT, "OPTIONS", "GET"),
        (CREATE, "GET", "POST"),
        (CREATE, "PUT", "POST"),
        (CREATE, "DELETE", "POST"),
        (CREATE, "OPTIONS", "POST"),
        (REPLACE, "GET", "PUT"),
        (REPLACE, "POST", "PUT"),
        (REPLACE, "DELETE", "PUT"),
        (REPLACE, "PATCH", "PUT"),
        (REPLACE, "OPTIONS", "PUT"),
    ] {
        let response = capture(app.clone(), request(method, path, untouched())).await;
        error(
            &response,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "not_attempted",
        );
        assert_eq!(response.headers[header::ALLOW], allow);
    }
    for (path, allow) in [
        (INDEX, Some("GET")),
        (CONTENT, Some("GET")),
        (CREATE, Some("POST")),
        (REPLACE, Some("PUT")),
        ("/absent", None),
    ] {
        let response = capture(app.clone(), request("HEAD", path, untouched())).await;
        assert_eq!(
            response.status,
            if allow.is_some() {
                StatusCode::METHOD_NOT_ALLOWED
            } else {
                StatusCode::NOT_FOUND
            }
        );
        assert_eq!(
            response
                .headers
                .get(header::ALLOW)
                .and_then(|v| v.to_str().ok()),
            allow
        );
        assert_eq!(response.headers[header::CONTENT_LENGTH], "0");
        assert!(response.json.is_null());
        private(&response);
    }
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn workspace_index_keeps_strict_query_cookie_origin_and_empty_body_policy() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let app = app(backend.clone()).await;
    for query in [
        "?",
        "?owner=someone",
        "?limit=0",
        "?limit=01",
        "?limit=33",
        "?limit=1&limit=2",
        "?limit=%31",
    ] {
        error(
            &capture(
                app.clone(),
                request("GET", &format!("{INDEX}{query}"), untouched()),
            )
            .await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    for query in [
        "",
        "?limit=1",
        "?limit=32",
        "?after=00000000-0000-4000-8000-000000000001",
    ] {
        dispatched(
            &capture(
                app.clone(),
                request("GET", &format!("{INDEX}{query}"), Body::empty()),
            )
            .await,
        );
    }
    for name in ["x-cyanrex-platform-request", "origin"] {
        let mut req = request("GET", INDEX, untouched());
        req.headers_mut()
            .append(name, HeaderValue::from_static("1"));
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
    }
    let mut req = request("GET", INDEX, untouched());
    req.headers_mut().remove(header::COOKIE);
    error(
        &capture(app.clone(), req).await,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "not_attempted",
    );
    let mut req = request("GET", INDEX, untouched());
    req.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer synthetic"),
    );
    error(
        &capture(app.clone(), req).await,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "not_attempted",
    );
    let mut req = request("GET", INDEX, Body::empty());
    req.headers_mut().remove(header::ORIGIN);
    dispatched(&capture(app.clone(), req).await);
    error(
        &capture(app, request("GET", INDEX, Body::from("x"))).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn workspace_content_keeps_raw_identity_and_current_private_read_transport() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let app = app(backend.clone()).await;
    for path in [
        format!("{CONTENT}?"),
        format!("{CONTENT}?owner=another"),
        format!("{CONTENT}?namespace=another"),
        "/platform/v1/tasks/00000000-0000-0000-0000-000000000000".into(),
        "/platform/v1/tasks/E3698554-8EB3-4B6D-884E-8074E1555136".into(),
    ] {
        error(
            &capture(app.clone(), request("GET", &path, untouched())).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    let mut req = request("GET", CONTENT, untouched());
    req.headers_mut().remove(header::COOKIE);
    error(
        &capture(app.clone(), req).await,
        StatusCode::UNAUTHORIZED,
        "invalid_session",
        "not_attempted",
    );
    let mut req = request("GET", CONTENT, untouched());
    req.headers_mut().remove("x-cyanrex-platform-request");
    error(
        &capture(app.clone(), req).await,
        StatusCode::FORBIDDEN,
        "request_forbidden",
        "not_attempted",
    );
    let mut req = request("GET", CONTENT, Body::empty());
    req.headers_mut().remove(header::ORIGIN);
    dispatched(&capture(app.clone(), req).await);
    error(
        &capture(app, request("GET", CONTENT, Body::from("{}"))).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn workspace_response_leases_are_shared_by_clones_and_independent_between_six_lanes() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let state = state(backend.clone(), [1, 1, 1, 1, 1, 1]).await;
    let first = build_platform_workspace_router(state.clone());
    let second = build_platform_workspace_router(state);
    let held_index = first
        .clone()
        .oneshot(request("GET", INDEX, Body::empty()))
        .await
        .unwrap();
    let held_session = second
        .clone()
        .oneshot(request("GET", SESSION, Body::empty()))
        .await
        .unwrap();
    let held_login = first
        .clone()
        .oneshot(request("POST", LOGIN, login_body()))
        .await
        .unwrap();
    let held_content = second
        .clone()
        .oneshot(request("GET", CONTENT, Body::empty()))
        .await
        .unwrap();
    let held_create = first
        .clone()
        .oneshot(request("POST", CREATE, create_body()))
        .await
        .unwrap();
    let held_replace = second
        .clone()
        .oneshot(request("PUT", REPLACE, replace_body()))
        .await
        .unwrap();
    assert_eq!(held_index.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(held_session.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(held_login.status(), StatusCode::OK);
    assert_eq!(held_content.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(held_create.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(held_replace.status(), StatusCode::SERVICE_UNAVAILABLE);
    for (method, path) in [
        ("GET", INDEX),
        ("GET", SESSION),
        ("POST", LOGOUT),
        ("POST", LOGIN),
        ("GET", CONTENT),
        ("POST", CREATE),
        ("PUT", REPLACE),
    ] {
        error(
            &capture(second.clone(), request(method, path, untouched())).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        );
    }
    // EOF releases only the index lane; the unsent lifecycle and login responses keep theirs.
    dispatched(&capture_response(held_index).await);
    dispatched(&capture(second.clone(), request("GET", INDEX, Body::empty())).await);
    for (method, path) in [
        ("GET", SESSION),
        ("POST", LOGIN),
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
    // Drop also releases just its own lane, including an admitted error response.
    drop(held_session);
    dispatched(&capture(second.clone(), request("POST", LOGOUT, Body::empty())).await);
    error(
        &capture(first.clone(), request("POST", LOGIN, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    issued(&capture_response(held_login).await);
    issued(&capture(second.clone(), request("POST", LOGIN, login_body())).await);
    error(
        &capture(second.clone(), request("GET", CONTENT, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    dispatched(&capture_response(held_content).await);
    dispatched(&capture(second.clone(), request("GET", CONTENT, Body::empty())).await);
    error(
        &capture(second.clone(), request("POST", CREATE, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    dispatched(&capture_response(held_create).await);
    dispatched(&capture(second.clone(), request("POST", CREATE, create_body())).await);
    error(
        &capture(second.clone(), request("PUT", REPLACE, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    dispatched(&capture_response(held_replace).await);
    dispatched(&capture(second, request("PUT", REPLACE, replace_body())).await);
    assert_eq!(backend.calls(), 2);
}
