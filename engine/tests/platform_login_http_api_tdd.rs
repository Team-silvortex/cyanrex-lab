#![cfg(unix)]
//! API composition only; a constructed router neither listens nor proves real authentication.
use axum::Router;
use cyanrex_engine::{
    platform_host::PlatformLoginConnector,
    platform_http::{
        build_session_login_router, SessionLoginHttpState, TaskContentHttpConfigError,
    },
};

#[test]
fn platform_login_http_consumes_the_explicit_issuer_and_keeps_shared_state_typed() {
    let _: fn(PlatformLoginConnector) -> Result<SessionLoginHttpState, TaskContentHttpConfigError> =
        SessionLoginHttpState::new;
    let _: fn(
        PlatformLoginConnector,
        usize,
    ) -> Result<SessionLoginHttpState, TaskContentHttpConfigError> =
        SessionLoginHttpState::with_capacity;
    let _: fn(SessionLoginHttpState) -> Router = build_session_login_router;
    fn shared<T: Clone + Send + Sync>() {}
    shared::<SessionLoginHttpState>();
}
