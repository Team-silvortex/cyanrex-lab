#![cfg(target_os = "linux")]

// Import only the existing panic-safe setup/cleanup fixtures, never their test modules.
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
#[path = "platform_storage_check/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[path = "platform_storage_check/failures.rs"]
mod failures;
#[allow(dead_code)]
#[path = "platform_storage_markers/fixture.rs"]
mod marker_fixture;
#[path = "platform_storage_check/process.rs"]
mod process;
#[path = "platform_storage_check/support.rs"]
mod support;
#[path = "platform_storage_check/waits.rs"]
mod waits;
