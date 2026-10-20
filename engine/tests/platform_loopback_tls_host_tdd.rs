#![cfg(target_os = "linux")]
//! Disposable real CLI/SCRAM/HTTPS evidence, not deployed LAN or browser receipt.
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
#[path = "platform_loopback_tls_host/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[path = "platform_serve_loopback_tls_defaults/certificates.rs"]
mod certificates;
#[allow(dead_code)]
#[path = "platform_loopback_host/child.rs"]
mod child;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_loopback_tls_host/faults.rs"]
mod faults;
#[path = "platform_loopback_tls_host/fixture.rs"]
mod fixture;
#[path = "platform_loopback_tls_host/http.rs"]
mod http;
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
#[path = "platform_loopback_tls_host/notify.rs"]
mod notify;
#[allow(dead_code, unused_imports)]
#[path = "platform_loopback_host/fixture.rs"]
mod plain_fixture;
#[allow(dead_code)]
#[path = "platform_loopback_host/http.rs"]
mod plain_http;
#[allow(dead_code)]
#[path = "platform_notify_defaults/receiver.rs"]
mod receiver;
#[path = "platform_loopback_tls_host/startup.rs"]
mod startup;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
#[allow(dead_code)]
#[path = "platform_storage_check/support.rs"]
mod wait_helpers;
#[path = "platform_loopback_tls_host/waits.rs"]
mod waits;
