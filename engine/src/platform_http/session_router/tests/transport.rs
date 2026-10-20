use super::super::super::build_platform_session_router;
use super::support::*;
use crate::{
    platform_http::{config, TaskContentHttpConfigError},
    services::auth_service::durable_source::DurableAuthError,
};
use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode},
};

#[tokio::test]
async fn combined_exact_routes_methods_queries_and_head_keep_one_private_fallback() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let app = app(backend.clone()).await;
    for path in [
        "/",
        "/auth/login",
        "/platform/v1/session/",
        "/platform/v1/session/login/",
        "/platform/v1/session/logout/",
        "/platform/v1/session/extra",
        "/platform/v1/session/%6cogin",
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
        (LOGIN, "OPTIONS", "POST"),
        (SESSION, "POST", "GET"),
        (SESSION, "PUT", "GET"),
        (LOGOUT, "GET", "POST"),
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
    for (path, method) in [(LOGIN, "POST"), (SESSION, "GET"), (LOGOUT, "POST")] {
        for query in ["?", "?schema_version=1"] {
            error(
                &capture(
                    app.clone(),
                    request(method, &format!("{path}{query}"), untouched()),
                )
                .await,
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "not_attempted",
            );
        }
    }
    for (path, allow) in [
        (LOGIN, Some("POST")),
        (SESSION, Some("GET")),
        (LOGOUT, Some("POST")),
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
        assert!(!response.headers.contains_key(header::SET_COOKIE));
        private(&response);
    }
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn combined_header_policy_stays_distinct_for_login_query_and_logout() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let app = app(backend.clone()).await;
    for (method, path) in [("POST", LOGIN), ("GET", SESSION), ("POST", LOGOUT)] {
        for name in ["x-cyanrex-platform-request", "origin"] {
            let mut req = request(method, path, untouched());
            req.headers_mut()
                .append(name, HeaderValue::from_static("1"));
            error(
                &capture(app.clone(), req).await,
                StatusCode::FORBIDDEN,
                "request_forbidden",
                "not_attempted",
            );
        }
        let mut req = request(method, path, untouched());
        req.headers_mut().remove("x-cyanrex-platform-request");
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
        let mut req = request(method, path, untouched());
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
    }
    for path in [LOGIN, LOGOUT] {
        let mut req = request("POST", path, untouched());
        req.headers_mut().remove(header::ORIGIN);
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
    }
    // GET alone permits an absent Origin, but still requires the dedicated token and marker.
    let mut req = request("GET", SESSION, Body::empty());
    req.headers_mut().remove(header::ORIGIN);
    lifecycle_dispatched(&capture(app.clone(), req).await);
    for cookie in [
        "cyanrex_platform_session",
        "cyanrex_platform_session=",
        "cyanrex_platform_session=broken",
    ] {
        let mut req = request("POST", LOGIN, untouched());
        req.headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        error(
            &capture(app.clone(), req).await,
            StatusCode::CONFLICT,
            "session_present",
            "not_attempted",
        );
        for (method, path) in [("GET", SESSION), ("POST", LOGOUT)] {
            let mut req = request(method, path, untouched());
            req.headers_mut()
                .insert(header::COOKIE, cookie.parse().unwrap());
            error(
                &capture(app.clone(), req).await,
                StatusCode::UNAUTHORIZED,
                "invalid_session",
                "not_attempted",
            );
        }
    }
    let mut req = request("POST", LOGIN, login_body());
    req.headers_mut().insert(
        header::COOKIE,
        HeaderValue::from_static("legacy_session=ignored"),
    );
    issued(&capture(app, req).await, true);
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn combined_body_policy_routes_login_json_but_lifecycle_accepts_only_empty() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let app = app(backend.clone()).await;
    error(
        &capture(app.clone(), request("POST", LOGIN, Body::from("{}"))).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    issued(
        &capture(app.clone(), request("POST", LOGIN, login_body())).await,
        true,
    );
    for (method, path) in [("GET", SESSION), ("POST", LOGOUT)] {
        let mut empty = request(method, path, Body::empty());
        empty.headers_mut().remove(header::CONTENT_TYPE);
        lifecycle_dispatched(&capture(app.clone(), empty).await);
        error(
            &capture(app.clone(), request(method, path, Body::from("{}"))).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn combined_dispatched_failures_never_issue_clear_or_reflect_a_cookie() {
    for (failure, status, code) in [
        (
            DurableAuthError::InvalidCredentials,
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
        ),
        (
            DurableAuthError::RateLimited,
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
        ),
        (
            DurableAuthError::StorageUnavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
        (
            DurableAuthError::SourceMismatch,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
    ] {
        let backend = LoginBackend::new(LoginOutcome::Error(failure));
        let app = app(backend.clone()).await;
        error(
            &capture(app.clone(), request("POST", LOGIN, login_body())).await,
            status,
            code,
            "unconfirmed",
        );
        for (method, path) in [("GET", SESSION), ("POST", LOGOUT)] {
            lifecycle_dispatched(&capture(app.clone(), request(method, path, Body::empty())).await);
        }
        assert_eq!(backend.calls(), 1);
    }
    for token in ["", "bad-token", "synthetic-secret\r\nSet-Cookie: injected"] {
        let backend = LoginBackend::new(LoginOutcome::Token(token));
        error(
            &capture(
                app(backend.clone()).await,
                request("POST", LOGIN, login_body()),
            )
            .await,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        );
        assert_eq!(backend.calls(), 1);
    }
}

#[tokio::test]
async fn combined_cookie_issuance_uses_only_trusted_origin_not_forwarding_headers() {
    for (origin, secure) in [
        (ORIGIN, true),
        ("http://127.0.0.1:3000", false),
        ("http://[::1]:3000", false),
    ] {
        let backend = LoginBackend::new(LoginOutcome::Success);
        let app = build_platform_session_router(state(backend.clone(), origin, 4, 4).await);
        let mut req = request("POST", LOGIN, login_body());
        req.headers_mut()
            .insert(header::ORIGIN, origin.parse().unwrap());
        req.headers_mut()
            .insert(header::HOST, HeaderValue::from_static("attacker.example"));
        req.headers_mut()
            .insert("x-forwarded-proto", HeaderValue::from_static("https"));
        req.headers_mut().insert(
            "forwarded",
            HeaderValue::from_static("proto=http;host=attacker.example"),
        );
        issued(&capture(app, req).await, secure);
        assert_eq!(backend.calls(), 1);
    }
}

#[test]
fn combined_shared_configuration_policy_rejects_either_bad_budget_after_origin() {
    // Pure policy only. Actual opaque-storage constructor, default 4+4 and original Arc binding
    // are checked by the public API/structure and real-SQL tests, not this synthetic fixture.
    for login in [1, 4, 16] {
        for lifecycle in [1, 4, 16] {
            assert!(config::validate(ORIGIN, &[login, lifecycle]).is_ok());
        }
    }
    for invalid in [0, 17, usize::MAX] {
        for capacities in [[invalid, 4], [4, invalid], [invalid, invalid]] {
            assert!(matches!(
                config::validate(ORIGIN, &capacities),
                Err(TaskContentHttpConfigError::InvalidCapacity)
            ));
            assert!(matches!(
                config::validate("http://platform.example", &capacities),
                Err(TaskContentHttpConfigError::InvalidOrigin)
            ));
        }
    }
}
