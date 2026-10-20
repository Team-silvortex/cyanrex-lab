#![cfg(unix)]
//! Retained private whole-manifest edits, never Artifact publication or execution.

mod config {
    use cyanrex_engine::models::collaboration::WorkspaceRef;
    pub(super) fn scope() -> WorkspaceRef {
        WorkspaceRef {
            authority_id: uuid::Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: uuid::Uuid::from_u128(4202).try_into().unwrap(),
        }
    }
}
#[path = "platform_task_replace_connection/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "platform_loopback_host/child.rs"]
mod child;
#[path = "platform_task_replace_connection/concurrency.rs"]
mod concurrency;
#[path = "platform_task_replace_connection/content.rs"]
mod content;
#[allow(dead_code)]
#[path = "platform_task_content_connection/fixture.rs"]
mod content_fixture;
#[path = "platform_task_replace_connection/drift.rs"]
mod drift;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_task_replace_connection/faults.rs"]
mod faults;
#[path = "platform_task_replace_connection/fixture.rs"]
mod fixture;
#[path = "platform_task_replace_connection/guards.rs"]
mod guards;
#[cfg(target_os = "linux")]
#[path = "platform_task_replace_connection/host.rs"]
mod host;
#[path = "platform_task_replace_connection/http.rs"]
mod http;
#[allow(dead_code)]
#[path = "platform_task_index_connection/fixture.rs"]
mod index_fixture;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "platform_config_input/kernel.rs"]
mod kernel;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
#[path = "platform_marker_expectation/support.rs"]
mod marker_helpers;
#[path = "platform_task_replace_connection/ordered.rs"]
mod ordered;
#[path = "platform_task_replace_connection/permissions.rs"]
mod permissions;
#[path = "platform_task_index_connection/schema.rs"]
mod schema;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
#[path = "platform_task_replace_connection/waits.rs"]
mod waits;
