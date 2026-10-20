#![cfg(unix)]
//! Retained-storage owner metadata reads, not login, content reopening or a public listener.

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
#[path = "platform_task_index_connection/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "platform_loopback_host/child.rs"]
mod child;
#[path = "platform_task_index_connection/content.rs"]
mod content;
#[path = "platform_task_index_connection/drift.rs"]
mod drift;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_task_index_connection/fixture.rs"]
mod fixture;
#[cfg(target_os = "linux")]
#[path = "platform_task_index_connection/host.rs"]
mod host;
#[path = "platform_task_index_connection/http.rs"]
mod http;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "platform_config_input/kernel.rs"]
mod kernel;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "platform_marker_expectation/support.rs"]
mod marker_helpers;
#[path = "platform_task_index_connection/permissions.rs"]
mod permissions;
#[path = "platform_task_index_connection/schema.rs"]
mod schema;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
#[path = "platform_task_index_connection/waits.rs"]
mod waits;
