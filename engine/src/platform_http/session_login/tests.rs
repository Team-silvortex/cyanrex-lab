use super::test_support::*;
use super::{Backend as LoginBackend, SessionLoginHttpState};
use crate::platform_http::TaskContentHttpConfigError;
use axum::http::{header, HeaderValue, StatusCode};
use tower::ServiceExt;

#[path = "tests/bodies.rs"]
mod bodies;
#[path = "tests/rate.rs"]
mod rate;
#[path = "tests/results.rs"]
mod results;

#[tokio::test]
async fn login_http_success_is_minimal_private_and_issues_one_sensitive_cookie() {
    let backend = Backend::new(Outcome::Success);
    let response = capture(app(backend.clone()), request("POST", LOGIN, valid_body())).await;
    success(&response, true);
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn login_http_trusted_origin_and_capacity_are_canonical_and_bounded() {
    for origin in [
        ORIGIN,
        "https://platform.example:8443",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://[::1]:3000",
    ] {
        assert!(SessionLoginHttpState::configured(
            LoginBackend::Test(Backend::new(Outcome::Success)),
            origin,
            4
        )
        .is_ok());
    }
    for origin in [
        "",
        "null",
        "*",
        "http://192.168.1.1",
        "http://platform.example",
        "https://PLATFORM.example",
        "https://platform.example/",
        "https://platform.example:443",
        "https://platform.example/path",
        "https://user@platform.example",
        "https://platform.example?secret=a",
        "https://platform.example#fragment",
        " https://platform.example",
        "https://*.example",
    ] {
        assert!(matches!(
            SessionLoginHttpState::configured(
                LoginBackend::Test(Backend::new(Outcome::Success)),
                origin,
                4
            ),
            Err(TaskContentHttpConfigError::InvalidOrigin)
        ));
    }
    for capacity in [0, 17, usize::MAX] {
        assert!(matches!(
            SessionLoginHttpState::configured(
                LoginBackend::Test(Backend::new(Outcome::Success)),
                ORIGIN,
                capacity
            ),
            Err(TaskContentHttpConfigError::InvalidCapacity)
        ));
    }
    for capacity in [1, 4, 16] {
        let _ = state(Backend::new(Outcome::Success), ORIGIN, capacity);
    }
}

#[tokio::test]
async fn login_http_exact_route_method_query_and_head_guards_never_read_the_body() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
    for path in [
        "/",
        "/auth/login",
        "/platform/v1/session",
        "/platform/v1/session/logout",
        "/platform/v1/session/login/",
        "/platform/v1/session/login/extra",
        "/platform/v1/session/%6cogin",
    ] {
        let response = capture(app.clone(), request("POST", path, untouched())).await;
        error(
            &response,
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        );
    }
    for method in ["GET", "PUT", "DELETE", "OPTIONS", "PATCH"] {
        let response = capture(app.clone(), request(method, LOGIN, untouched())).await;
        error(
            &response,
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "not_attempted",
        );
        assert_eq!(response.headers[header::ALLOW], "POST");
    }
    let response = capture(
        app.clone(),
        request("POST", &format!("{LOGIN}?password=synthetic"), untouched()),
    )
    .await;
    error(
        &response,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    let response = app
        .oneshot(request("HEAD", LOGIN, untouched()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(response.headers()[header::ALLOW], "POST");
    assert_eq!(response.headers()[header::CONTENT_LENGTH], "0");
    let response = capture_response(response).await;
    assert!(response.json.is_null());
    private(&response);
    assert!(!response.headers.contains_key(header::SET_COOKIE));
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn login_http_origin_marker_and_authorization_fail_before_body_or_backend() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
    for name in ["origin", "x-cyanrex-platform-request"] {
        let mut req = request("POST", LOGIN, untouched());
        req.headers_mut().remove(name);
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
        let mut req = request("POST", LOGIN, untouched());
        req.headers_mut()
            .append(name, HeaderValue::from_static("1"));
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
    }
    for origin in [
        "null",
        "https://other.example",
        "https://platform.example/",
        "https://PLATFORM.example",
        "https://platform.example:443",
    ] {
        let mut req = request("POST", LOGIN, untouched());
        req.headers_mut()
            .insert(header::ORIGIN, origin.parse().unwrap());
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
    }
    for value in ["0", "true", " 1", "1 ", "1, 1"] {
        let mut req = request("POST", LOGIN, untouched());
        req.headers_mut()
            .insert("x-cyanrex-platform-request", value.parse().unwrap());
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
    }
    for value in ["", "Bearer synthetic"] {
        let mut req = request("POST", LOGIN, untouched());
        req.headers_mut()
            .insert(header::AUTHORIZATION, value.parse().unwrap());
        error(
            &capture(app.clone(), req).await,
            StatusCode::UNAUTHORIZED,
            "invalid_session",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
}

#[tokio::test]
async fn login_http_existing_platform_cookie_is_never_replaced_or_adopted() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
    for cookie in [
        "cyanrex_platform_session".to_owned(),
        "cyanrex_platform_session=".to_owned(),
        "cyanrex_platform_session=broken".to_owned(),
        format!("cyanrex_platform_session={TOKEN}"),
        format!("cyanrex_platform_session=\"{TOKEN}\""),
        format!("cyanrex_platform_session={TOKEN}; cyanrex_platform_session={TOKEN}"),
        "unrelated=x; cyanrex_platform_session = value".to_owned(),
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
    }
    let mut req = request("POST", LOGIN, untouched());
    req.headers_mut()
        .append(header::COOKIE, HeaderValue::from_static("unrelated=value"));
    req.headers_mut().append(
        header::COOKIE,
        HeaderValue::from_static("cyanrex_platform_session=invalid"),
    );
    error(
        &capture(app.clone(), req).await,
        StatusCode::CONFLICT,
        "session_present",
        "not_attempted",
    );
    for bytes in [
        b"cyanrex_platform_session=\x80".as_slice(),
        b"cyanrex_platform_session = \xff; unrelated=ok".as_slice(),
    ] {
        let mut req = request("POST", LOGIN, untouched());
        req.headers_mut()
            .insert(header::COOKIE, HeaderValue::from_bytes(bytes).unwrap());
        error(
            &capture(app.clone(), req).await,
            StatusCode::CONFLICT,
            "session_present",
            "not_attempted",
        );
    }
    for dedicated_first in [true, false] {
        let mut req = request("POST", LOGIN, untouched());
        let dedicated = HeaderValue::from_static("cyanrex_platform_session");
        let unrelated = HeaderValue::from_bytes(b"unrelated=\x80").unwrap();
        let values = if dedicated_first {
            [dedicated, unrelated]
        } else {
            [unrelated, dedicated]
        };
        for value in values {
            req.headers_mut().append(header::COOKIE, value);
        }
        error(
            &capture(app.clone(), req).await,
            StatusCode::CONFLICT,
            "session_present",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 0);
    let mut req = request("POST", LOGIN, valid_body());
    req.headers_mut().insert(
        header::COOKIE,
        HeaderValue::from_static("cyanrex_session=legacy; unrelated=value"),
    );
    success(&capture(app, req).await, true);
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn login_http_aggregate_cookie_budget_precedes_body_and_backend() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
    let mut req = request("POST", LOGIN, untouched());
    for name in ["first", "second"] {
        req.headers_mut().append(
            header::COOKIE,
            format!("{name}={}", "x".repeat(4096)).parse().unwrap(),
        );
    }
    error(
        &capture(app.clone(), req).await,
        StatusCode::PAYLOAD_TOO_LARGE,
        "request_too_large",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);

    let mut req = request("POST", LOGIN, untouched());
    req.headers_mut().append(
        header::COOKIE,
        HeaderValue::from_static("cyanrex_platform_session=broken"),
    );
    req.headers_mut().append(
        header::COOKIE,
        format!("padding={}", "x".repeat(8192)).parse().unwrap(),
    );
    error(
        &capture(app.clone(), req).await,
        StatusCode::PAYLOAD_TOO_LARGE,
        "request_too_large",
        "not_attempted",
    );

    let mut req = request("POST", LOGIN, untouched());
    req.headers_mut().insert(
        header::COOKIE,
        HeaderValue::from_bytes(b"unrelated=\xff").unwrap(),
    );
    error(
        &capture(app.clone(), req).await,
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);

    let mut req = request("POST", LOGIN, valid_body());
    req.headers_mut().insert(
        header::COOKIE,
        format!("padding={}", "x".repeat(8192 - "padding=".len()))
            .parse()
            .unwrap(),
    );
    success(&capture(app, req).await, true);
    assert_eq!(backend.calls(), 1);
}
