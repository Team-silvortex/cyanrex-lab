#![cfg(target_os = "linux")]

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
#[path = "platform_marker_expectation/behavior.rs"]
mod behavior;
#[path = "platform_marker_expectation/boundaries.rs"]
mod boundaries;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[path = "platform_marker_expectation/cli.rs"]
mod cli;
#[path = "platform_marker_expectation/diagnostic.rs"]
mod diagnostic;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[allow(dead_code)]
#[path = "platform_storage_markers/fixture.rs"]
mod marker_fixture;
#[allow(dead_code)]
#[path = "platform_storage_check/process.rs"]
mod process_helpers;
#[path = "platform_marker_expectation/support.rs"]
mod support;
