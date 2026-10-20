use super::super::{build_session_login_router, test_support::*};
use axum::{
    body::Body,
    http::{HeaderValue, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn login_http_rate_is_shared_global_64_not_username_or_ip_and_separate_states_are_independent(
) {
    let backend = Backend::new(Outcome::Success);
    let state = state(backend.clone(), ORIGIN, 4);
    let first = build_session_login_router(state.clone());
    let second = build_session_login_router(state);
    for index in 0..64 {
        let mut value = valid_json();
        value["username"] = json!(format!("synthetic-{index}"));
        let mut req = request("POST", LOGIN, Body::from(value.to_string()));
        req.headers_mut().insert(
            "x-forwarded-for",
            format!("192.0.2.{}", index + 1).parse().unwrap(),
        );
        success(
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
            &capture(app, request("POST", LOGIN, untouched())).await,
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "not_attempted",
        );
    }
    assert_eq!(backend.calls(), 64);
    success(
        &capture(app(backend.clone()), request("POST", LOGIN, valid_body())).await,
        true,
    );
    assert_eq!(backend.calls(), 65);
}

#[tokio::test]
async fn login_http_rate_counts_invalid_body_attempts_without_refunds() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
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

#[tokio::test]
async fn login_http_rate_is_charged_before_busy_capacity_and_not_refunded_on_response_drop() {
    let backend = Backend::new(Outcome::Success);
    let app = build_session_login_router(state(backend.clone(), ORIGIN, 1));
    let response = app
        .clone()
        .oneshot(request("POST", LOGIN, valid_body()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
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
    drop(response);
    error(
        &capture(app, request("POST", LOGIN, untouched())).await,
        StatusCode::TOO_MANY_REQUESTS,
        "rate_limited",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn login_http_cancelled_admitted_request_frees_capacity_without_refunding_rate() {
    let backend = Backend::blocking(Outcome::Success);
    let app = build_session_login_router(state(backend.clone(), ORIGIN, 1));
    let task = tokio::spawn(app.clone().oneshot(request("POST", LOGIN, valid_body())));
    backend.entered().await;
    task.abort();
    assert!(matches!(task.await,Err(error) if error.is_cancelled()));
    // This fake cancellation checks HTTP leases only, not termination of real KDF jobs.
    backend.release(1);
    for _ in 1..64 {
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
    assert_eq!(backend.calls(), 1);
}

#[tokio::test]
async fn login_http_cheap_route_header_and_existing_cookie_rejections_do_not_charge_rate() {
    let backend = Backend::new(Outcome::Success);
    let app = app(backend.clone());
    error(
        &capture(app.clone(), request("POST", "/auth/login", untouched())).await,
        StatusCode::NOT_FOUND,
        "not_found",
        "not_attempted",
    );
    let mut req = request("POST", LOGIN, untouched());
    req.headers_mut().remove("origin");
    error(
        &capture(app.clone(), req).await,
        StatusCode::FORBIDDEN,
        "request_forbidden",
        "not_attempted",
    );
    let mut req = request("POST", LOGIN, untouched());
    req.headers_mut().insert(
        "cookie",
        HeaderValue::from_static("cyanrex_platform_session=broken"),
    );
    error(
        &capture(app.clone(), req).await,
        StatusCode::CONFLICT,
        "session_present",
        "not_attempted",
    );
    for _ in 0..64 {
        success(
            &capture(app.clone(), request("POST", LOGIN, valid_body())).await,
            true,
        );
    }
    error(
        &capture(app, request("POST", LOGIN, untouched())).await,
        StatusCode::TOO_MANY_REQUESTS,
        "rate_limited",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 64);
}
