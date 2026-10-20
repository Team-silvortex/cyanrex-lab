#![cfg(unix)]
//! One retained binding across unmounted login/current-Session/logout HTTP operations.
use cyanrex_engine::{
    platform_host::PlatformStorageConnector,
    platform_http::{
        build_platform_session_router, PlatformSessionHttpState, TaskContentHttpConfigError,
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
#[path = "platform_session_router/admission.rs"]
mod admission;
#[path = "platform_session_router/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_session_router/faults.rs"]
mod faults;
#[path = "platform_session_router/fixture.rs"]
mod fixture;
#[path = "platform_session_router/lifetime.rs"]
mod lifetime;
#[allow(dead_code)]
#[path = "platform_login_connection/fixture.rs"]
mod login_fixture;
#[allow(dead_code, unused_imports)]
#[path = "platform_login_http/fixture.rs"]
mod login_http_helpers;
#[allow(dead_code)]
#[path = "platform_storage_markers/fixture.rs"]
mod marker_fixture;
#[allow(dead_code)]
#[path = "platform_marker_expectation/support.rs"]
mod marker_helpers;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;

#[test]
fn platform_session_router_consumes_one_binding_and_separate_bounded_capacities() {
    let _: fn(
        PlatformStorageConnector,
    ) -> Result<PlatformSessionHttpState, TaskContentHttpConfigError> =
        PlatformSessionHttpState::new;
    let _: fn(
        PlatformStorageConnector,
        usize,
        usize,
    ) -> Result<PlatformSessionHttpState, TaskContentHttpConfigError> =
        PlatformSessionHttpState::with_capacities;
    let _: fn(PlatformSessionHttpState) -> axum::Router = build_platform_session_router;
    fn traits<T: Clone + Send + Sync>() {}
    traits::<PlatformSessionHttpState>();
}
