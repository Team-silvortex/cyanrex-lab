use super::super::{build_session_login_router, test_support::*};
use crate::{
    models::collaboration::AuthorityId,
    platform_http::{build_session_lifecycle_router, SessionLifecycleHttpState},
    services::auth_service::durable_source::{DurableAuthError, DurableAuthSource},
};
use axum::http::{header, HeaderValue, StatusCode};
use sqlx_postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn login_http_cookie_security_comes_only_from_trusted_origin_not_proxy_headers() {
    for (origin, secure) in [
        (ORIGIN, true),
        ("https://platform.example:8443", true),
        ("http://localhost:3000", false),
        ("http://127.0.0.1:3000", false),
        ("http://[::1]:3000", false),
    ] {
        let backend = Backend::new(Outcome::Success);
        let app = build_session_login_router(state(backend.clone(), origin, 4));
        let mut req = request("POST", LOGIN, valid_body());
        req.headers_mut()
            .insert(header::ORIGIN, origin.parse().unwrap());
        req.headers_mut()
            .insert(header::HOST, HeaderValue::from_static("untrusted.example"));
        req.headers_mut().insert(
            "x-forwarded-proto",
            if secure {
                HeaderValue::from_static("http")
            } else {
                HeaderValue::from_static("https")
            },
        );
        req.headers_mut().insert(
            "forwarded",
            HeaderValue::from_static("proto=http;host=untrusted.example"),
        );
        success(&capture(app, req).await, secure);
        assert_eq!(backend.calls(), 1);
    }
}

#[tokio::test]
async fn login_http_backend_errors_are_unconfirmed_private_and_never_change_cookies() {
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
        (
            DurableAuthError::UnsupportedSchema,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
        (
            DurableAuthError::InvalidRecord,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
        (
            DurableAuthError::LegacyDataPresent,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
        (
            DurableAuthError::InvalidInput,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
        (
            DurableAuthError::AccountExists,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        ),
    ] {
        let backend = Backend::new(Outcome::Error(failure));
        error(
            &capture(app(backend.clone()), request("POST", LOGIN, valid_body())).await,
            status,
            code,
            "unconfirmed",
        );
        assert_eq!(backend.calls(), 1);
    }
}

#[tokio::test]
async fn login_http_invalid_backend_token_or_raw_secret_fails_closed_without_header_injection() {
    for token in [
        "",
        PASSWORD,
        "raw-secret\r\nSet-Cookie:other=injected",
        "AE0B35CD-C9B8-413C-A892-854B15959F41",
        "ae0b35cdc9b8413ca892854b15959f41",
        "00000000-0000-0000-0000-000000000000",
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
    ] {
        let backend = Backend::new(Outcome::Token(token));
        error(
            &capture(app(backend.clone()), request("POST", LOGIN, valid_body())).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "unconfirmed",
        );
        assert_eq!(backend.calls(), 1);
    }
}

#[tokio::test]
async fn login_http_response_lease_is_shared_until_body_completion_or_drop() {
    let backend = Backend::new(Outcome::Success);
    let state = state(backend.clone(), ORIGIN, 1);
    let first = build_session_login_router(state.clone());
    let second = build_session_login_router(state);
    let response = first
        .clone()
        .oneshot(request("POST", LOGIN, valid_body()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    error(
        &capture(second.clone(), request("POST", LOGIN, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 1);
    success(&capture_response(response).await, true);
    let response = second
        .clone()
        .oneshot(request("POST", LOGIN, valid_body()))
        .await
        .unwrap();
    assert_eq!(backend.calls(), 2);
    error(
        &capture(first.clone(), request("POST", LOGIN, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    drop(response);
    success(
        &capture(first, request("POST", LOGIN, valid_body())).await,
        true,
    );
    assert_eq!(backend.calls(), 3);
}

#[tokio::test]
async fn login_http_does_not_extend_the_old_lifecycle_router_with_issuance() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let source =
        DurableAuthSource::new(pool, AuthorityId::try_from(Uuid::from_u128(4201)).unwrap());
    let app =
        build_session_lifecycle_router(SessionLifecycleHttpState::new(source, ORIGIN).unwrap());
    error(
        &capture(app, request("POST", LOGIN, untouched())).await,
        StatusCode::NOT_FOUND,
        "not_found",
        "not_attempted",
    );
}
