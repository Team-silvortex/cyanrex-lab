#[path = "test_support.rs"]
mod support;

use crate::platform_http::{
    build_session_lifecycle_router, build_session_login_router,
    session_login::test_support as login, SessionLifecycleHttpState,
};
use axum::{body::Body, http::StatusCode};
use support::*;

#[path = "tests/capacity.rs"]
mod capacity;
#[path = "tests/rate.rs"]
mod rate;
#[path = "tests/transport.rs"]
mod transport;

#[tokio::test]
async fn platform_session_router_composes_login_without_extending_old_routers() {
    let backend = LoginBackend::new(LoginOutcome::Success);
    let response = capture(
        app(backend.clone()).await,
        request("POST", LOGIN, login_body()),
    )
    .await;
    issued(&response, true);
    assert_eq!(backend.calls(), 1);

    // Standalone routers retain their previous ownership. Composition is explicit, not a change
    // to old builders or a global fallback which adopts the other family's routes.
    let old_login = build_session_login_router(login::state(backend.clone(), ORIGIN, 4));
    for (method, path) in [("GET", SESSION), ("POST", LOGOUT)] {
        error(
            &capture(old_login.clone(), request(method, path, untouched())).await,
            StatusCode::NOT_FOUND,
            "not_found",
            "not_attempted",
        );
    }
    let old_lifecycle = build_session_lifecycle_router(
        SessionLifecycleHttpState::new(closed_source().await, ORIGIN).unwrap(),
    );
    error(
        &capture(old_lifecycle, request("POST", LOGIN, untouched())).await,
        StatusCode::NOT_FOUND,
        "not_found",
        "not_attempted",
    );
    // Neither the known lifecycle paths above nor an arbitrary unknown path reaches this fake.
    error(
        &capture(
            app(backend.clone()).await,
            request("POST", "/auth/login", Body::empty()),
        )
        .await,
        StatusCode::NOT_FOUND,
        "not_found",
        "not_attempted",
    );
    assert_eq!(backend.calls(), 1);
}
