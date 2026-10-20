use super::super::super::build_platform_session_router;
use super::support::*;
use axum::{
    body::Body,
    http::{header, HeaderValue, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn combined_login_rate_is_shared_across_builds_but_not_lifecycle_or_separate_states() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let state = state(backend.clone(), ORIGIN, 4, 4).await;
    let first = build_platform_session_router(state.clone());
    let second = build_platform_session_router(state);
    for index in 0..64 {
        // Lifecycle calls use their own admission and must not consume the pre-auth login budget.
        lifecycle_dispatched(
            &capture(second.clone(), request("GET", SESSION, Body::empty())).await,
        );
        let mut req = request("POST", LOGIN, login_body());
        req.headers_mut().insert(
            "x-forwarded-for",
            format!("192.0.2.{}", index + 1).parse().unwrap(),
        );
        issued(
            &capture(
                if index % 2 == 0 {
                    first.clone()
                } else {
                    second.clone()
                },
                req,
            )
            .await,
            true,
        );
    }
    for app in [first, second] {
        error(
            &capture(app.clone(), request("POST", LOGIN, untouched())).await,
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "not_attempted",
        );
        lifecycle_dispatched(&capture(app, request("POST", LOGOUT, Body::empty())).await);
    }
    assert_eq!(backend.calls(), 64);
    issued(
        &capture(
            app(backend.clone()).await,
            request("POST", LOGIN, login_body()),
        )
        .await,
        true,
    );
    assert_eq!(backend.calls(), 65);
}

#[tokio::test]
async fn combined_rate_charges_capacity_rejections_before_body_and_never_refunds_drop() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let app = build_platform_session_router(state(backend.clone(), ORIGIN, 1, 1).await);
    let held = app
        .clone()
        .oneshot(request("POST", LOGIN, login_body()))
        .await
        .unwrap();
    assert_eq!(held.status(), StatusCode::OK);
    for _ in 1..64 {
        error(
            &capture(app.clone(), request("POST", LOGIN, untouched())).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        );
    }
    error(
        &capture(app.clone(), request("POST", LOGIN, untouched())).await,
        StatusCode::TOO_MANY_REQUESTS,
        "rate_limited",
        "not_attempted",
    );
    drop(held);
    error(
        &capture(app.clone(), request("POST", LOGIN, untouched())).await,
        StatusCode::TOO_MANY_REQUESTS,
        "rate_limited",
        "not_attempted",
    );
    lifecycle_dispatched(&capture(app, request("POST", LOGOUT, Body::empty())).await);
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn combined_route_header_and_existing_cookie_rejections_do_not_spend_login_rate() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let app = app(backend.clone()).await;
    for _ in 0..64 {
        error(
            &capture(app.clone(), request("POST", "/auth/login", untouched())).await,
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        );
        let mut req = request("POST", LOGIN, untouched());
        req.headers_mut().remove(header::ORIGIN);
        error(
            &capture(app.clone(), req).await,
            StatusCode::FORBIDDEN,
            "request_forbidden",
            "not_attempted",
        );
        let mut req = request("POST", LOGIN, untouched());
        req.headers_mut().insert(
            header::COOKIE,
            HeaderValue::from_static("cyanrex_platform_session=broken"),
        );
        error(
            &capture(app.clone(), req).await,
            StatusCode::CONFLICT,
            "session_present",
            "not_attempted",
        );
    }
    // Exact remaining budget verifies rejections did not create a hidden parallel/reset counter.
    for _ in 0..64 {
        error(
            &capture(app.clone(), request("POST", LOGIN, Body::from("{}"))).await,
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "not_attempted",
        );
    }
    error(
        &capture(app, request("POST", LOGIN, untouched())).await,
        StatusCode::TOO_MANY_REQUESTS,
        "rate_limited",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 0);
}
