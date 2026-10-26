#![cfg(target_os = "linux")]
//! Opt-in natural browser Session and exact-pin Task metadata edits on disposable SQL storage.
mod config {
    use cyanrex_engine::models::collaboration::WorkspaceRef;
    pub(super) fn scope() -> WorkspaceRef {
        WorkspaceRef {
            authority_id: uuid::Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: uuid::Uuid::from_u128(4202).try_into().unwrap(),
        }
    }
}
#[path = "platform_browser_task_metadata/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_browser_tasks/fixture.rs"]
mod browser_fixture;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code)]
#[path = "platform_loopback_host/child.rs"]
mod child;
#[allow(dead_code, unused_imports)]
#[path = "platform_task_content_connection/fixture.rs"]
mod content_fixture;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_browser_task_metadata/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_task_index_connection/fixture.rs"]
mod index_fixture;
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
#[path = "platform_browser_task_metadata/process.rs"]
mod process;
#[path = "platform_task_index_connection/schema.rs"]
mod schema;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
