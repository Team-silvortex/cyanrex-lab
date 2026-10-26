#![cfg(target_os = "linux")]
//! Opt-in real browser + original Session storage, never teaching authentication or Task saving.
mod config {
    use cyanrex_engine::models::collaboration::WorkspaceRef;
    pub(super) fn scope() -> WorkspaceRef {
        WorkspaceRef {
            authority_id: uuid::Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: uuid::Uuid::from_u128(4202).try_into().unwrap(),
        }
    }
}
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code)]
#[path = "platform_serve_loopback_tls_defaults/certificates.rs"]
mod certificates;
#[allow(dead_code)]
#[path = "platform_loopback_host/child.rs"]
mod child;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_browser_session/fixture.rs"]
mod fixture;
#[allow(dead_code, unused_imports)]
#[path = "platform_storage_open/fixture.rs"]
mod identity_helpers;
#[allow(dead_code)]
#[path = "platform_config_input/kernel.rs"]
mod kernel;
#[allow(dead_code)]
#[path = "platform_login_connection/fixture.rs"]
mod login_fixture;
#[allow(dead_code)]
#[path = "platform_storage_markers/fixture.rs"]
mod marker_fixture;
#[allow(dead_code)]
#[path = "platform_marker_expectation/support.rs"]
mod marker_helpers;
#[path = "platform_browser_session/plain.rs"]
mod plain;
#[allow(dead_code, unused_imports)]
#[path = "platform_loopback_host/fixture.rs"]
mod plain_fixture;
#[path = "platform_browser_session/process.rs"]
mod process;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
#[path = "platform_browser_session/tls.rs"]
mod tls;
