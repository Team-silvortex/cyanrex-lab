#![cfg(unix)]
//! Standalone retained-login HTTP transport, not a listener or live teaching application.
use cyanrex_engine::{
    platform_host::PlatformLoginConnector,
    platform_http::{
        build_session_login_router, SessionLoginHttpState, TaskContentHttpConfigError,
    },
};

mod config {
    use cyanrex_engine::models::collaboration::WorkspaceRef;
    use uuid::Uuid;
    pub(super) fn scope() -> WorkspaceRef {
        WorkspaceRef {
            authority_id: Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: Uuid::from_u128(4202).try_into().unwrap(),
        }
    }
}

#[path = "platform_login_http/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_login_http/faults.rs"]
mod faults;
#[path = "platform_login_http/fixture.rs"]
mod fixture;
#[path = "platform_login_http/lifetime.rs"]
mod lifetime;
#[allow(dead_code)]
#[path = "platform_login_connection/fixture.rs"]
mod login_fixture;
#[allow(dead_code)]
#[path = "platform_storage_markers/fixture.rs"]
mod marker_fixture;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;

#[test]
fn platform_login_http_state_consumes_only_the_retained_login_capability() {
    let _: fn(PlatformLoginConnector) -> Result<SessionLoginHttpState, TaskContentHttpConfigError> =
        SessionLoginHttpState::new;
    let _: fn(
        PlatformLoginConnector,
        usize,
    ) -> Result<SessionLoginHttpState, TaskContentHttpConfigError> =
        SessionLoginHttpState::with_capacity;
    let _: fn(SessionLoginHttpState) -> axum::Router = build_session_login_router;
    fn traits<T: Clone + Send + Sync>() {}
    traits::<SessionLoginHttpState>();
}
