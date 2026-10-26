#![cfg(unix)]
//! Explicit retained text publication only; no HTTP, host mount or browser saving.

mod config {
    use cyanrex_engine::models::collaboration::WorkspaceRef;
    pub(super) fn scope() -> WorkspaceRef {
        WorkspaceRef {
            authority_id: uuid::Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: uuid::Uuid::from_u128(4202).try_into().unwrap(),
        }
    }
}
#[path = "platform_text_artifact_connection/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code, unused_imports)]
#[path = "platform_task_content_connection/fixture.rs"]
mod content_fixture;
#[path = "platform_text_artifact_connection/drift.rs"]
mod drift;
#[path = "platform_text_artifact_connection/duplicates.rs"]
mod duplicates;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_text_artifact_connection/faults.rs"]
mod faults;
#[path = "platform_text_artifact_connection/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_task_index_connection/fixture.rs"]
mod index_fixture;
#[path = "platform_text_artifact_connection/lifecycle.rs"]
mod lifecycle;
#[path = "platform_text_artifact_connection/permissions.rs"]
mod permissions;
#[path = "platform_task_index_connection/schema.rs"]
mod schema;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
#[path = "platform_text_artifact_connection/waits.rs"]
mod waits;
