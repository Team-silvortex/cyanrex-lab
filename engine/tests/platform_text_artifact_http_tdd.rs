#![cfg(unix)]
//! Explicit retained publication HTTP only: no automatic host, SDK or browser wiring.

mod config {
    use cyanrex_engine::models::collaboration::WorkspaceRef;
    pub(super) fn scope() -> WorkspaceRef {
        WorkspaceRef {
            authority_id: uuid::Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: uuid::Uuid::from_u128(4202).try_into().unwrap(),
        }
    }
}
#[path = "platform_text_artifact_http/admission.rs"]
mod admission;
#[path = "platform_text_artifact_http/authority.rs"]
mod authority;
#[path = "platform_text_artifact_http/behavior.rs"]
mod behavior;
#[path = "platform_text_artifact_http/boundaries.rs"]
mod boundaries;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code, unused_imports)]
#[path = "platform_task_content_connection/fixture.rs"]
mod content_fixture;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_text_artifact_http/faults.rs"]
mod faults;
#[path = "platform_text_artifact_http/fixture.rs"]
mod fixture;
#[path = "platform_text_artifact_http/flow.rs"]
mod flow;
#[allow(dead_code)]
#[path = "platform_task_index_connection/fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "platform_text_artifact_connection/fixture.rs"]
mod retained_fixture;
#[path = "platform_task_index_connection/schema.rs"]
mod schema;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
