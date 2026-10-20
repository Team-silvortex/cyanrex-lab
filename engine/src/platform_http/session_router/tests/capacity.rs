use super::super::super::build_platform_session_router;
use super::support::*;
use axum::{body::Body, http::StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn combined_login_response_lease_is_shared_by_clones_but_does_not_block_logout() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let state = state(backend.clone(), ORIGIN, 1, 1).await;
    let first = build_platform_session_router(state.clone());
    let second = build_platform_session_router(state);
    let held = first
        .clone()
        .oneshot(request("POST", LOGIN, login_body()))
        .await
        .unwrap();
    assert_eq!(held.status(), StatusCode::OK);
    error(
        &capture(second.clone(), request("POST", LOGIN, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    // Reaching a closed backend establishes independent HTTP admission, not successful logout.
    for (method, path) in [("GET", SESSION), ("POST", LOGOUT)] {
        lifecycle_dispatched(&capture(second.clone(), request(method, path, Body::empty())).await);
    }
    issued(&capture_response(held).await, true); // EOF releases the shared login lease.
    issued(
        &capture(second, request("POST", LOGIN, login_body())).await,
        true,
    );
    assert_eq!(backend.calls(), 2);
}

#[tokio::test]
async fn combined_lifecycle_query_and_logout_share_one_lease_not_the_login_budget() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let state = state(backend.clone(), ORIGIN, 2, 1).await;
    let first = build_platform_session_router(state.clone());
    let second = build_platform_session_router(state);
    let held = first
        .clone()
        .oneshot(request("GET", SESSION, Body::empty()))
        .await
        .unwrap();
    assert_eq!(held.status(), StatusCode::SERVICE_UNAVAILABLE);
    for (method, path) in [("GET", SESSION), ("POST", LOGOUT)] {
        error(
            &capture(second.clone(), request(method, path, untouched())).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "capacity_exhausted",
            "not_attempted",
        );
    }
    let login_one = first
        .clone()
        .oneshot(request("POST", LOGIN, login_body()))
        .await
        .unwrap();
    let login_two = second
        .clone()
        .oneshot(request("POST", LOGIN, login_body()))
        .await
        .unwrap();
    assert_eq!(login_one.status(), StatusCode::OK);
    assert_eq!(login_two.status(), StatusCode::OK);
    error(
        &capture(first.clone(), request("POST", LOGIN, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    // Dropping an unconsumed error response releases only its lifecycle lease.
    drop(held);
    lifecycle_dispatched(&capture(second.clone(), request("POST", LOGOUT, Body::empty())).await);
    error(
        &capture(first.clone(), request("POST", LOGIN, untouched())).await,
        StatusCode::SERVICE_UNAVAILABLE,
        "capacity_exhausted",
        "not_attempted",
    );
    drop(login_one);
    issued(
        &capture(first, request("POST", LOGIN, login_body())).await,
        true,
    );
    drop(login_two);
    assert_eq!(backend.calls(), 3);
}

#[tokio::test]
async fn combined_cancelled_login_releases_http_admission_without_refunding_rate() {
    let backend = LoginBackend::blocking(LoginOutcome::Success);
    let app = build_platform_session_router(state(backend.clone(), ORIGIN, 1, 1).await);
    let task = tokio::spawn(app.clone().oneshot(request("POST", LOGIN, login_body())));
    backend.entered().await;
    // An in-flight login request, not just its unsent response, does not own the lifecycle slot.
    lifecycle_dispatched(&capture(app.clone(), request("POST", LOGOUT, Body::empty())).await);
    task.abort();
    assert!(matches!(task.await,Err(error) if error.is_cancelled()));
    // This fake cancellation tests HTTP leases only, not termination of real database/KDF work.
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
        &capture(app.clone(), request("POST", LOGIN, untouched())).await,
        StatusCode::TOO_MANY_REQUESTS,
        "rate_limited",
        "not_attempted",
    );
    lifecycle_dispatched(&capture(app, request("GET", SESSION, Body::empty())).await);
    assert_eq!(backend.calls(), 1);
}
