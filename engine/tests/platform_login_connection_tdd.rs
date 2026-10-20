#![cfg(unix)]

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

#[path = "platform_login_connection/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_login_connection/faults.rs"]
mod faults;
#[allow(dead_code)]
#[path = "platform_login_connection/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_storage_markers/fixture.rs"]
mod marker_fixture;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
#[path = "platform_login_connection/triggers.rs"]
mod triggers;
#[path = "platform_login_connection/waits.rs"]
mod waits;
