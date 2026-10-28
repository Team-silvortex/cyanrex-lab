#![cfg(unix)]
//! Unmounted Task-bound retained publication; no HTTP, browser save or Task mutation.

use cyanrex_engine::platform_host::{PlatformStorageConnector, PlatformTaskTextArtifactConnector};

mod config {
    use cyanrex_engine::models::collaboration::WorkspaceRef;
    pub(super) fn scope() -> WorkspaceRef {
        WorkspaceRef {
            authority_id: uuid::Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: uuid::Uuid::from_u128(4202).try_into().unwrap(),
        }
    }
}
#[path = "platform_task_text_artifact_connection/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code, unused_imports)]
#[path = "platform_task_content_connection/fixture.rs"]
mod content_fixture;
#[path = "platform_task_text_artifact_connection/drift.rs"]
mod drift;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_task_text_artifact_connection/faults.rs"]
mod faults;
#[path = "platform_task_text_artifact_connection/fixture.rs"]
mod fixture;
#[path = "platform_task_text_artifact_connection/guards.rs"]
mod guards;
#[allow(dead_code)]
#[path = "platform_task_index_connection/fixture.rs"]
mod index_fixture;
#[path = "platform_task_text_artifact_connection/permissions.rs"]
mod permissions;
#[allow(dead_code)]
#[path = "platform_text_artifact_connection/fixture.rs"]
mod retained_fixture;
#[path = "platform_task_index_connection/schema.rs"]
mod schema;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
#[path = "platform_task_text_artifact_connection/waits.rs"]
mod waits;

#[test]
fn task_text_artifact_connector_has_an_explicit_retained_storage_constructor() {
    fn shape(storage: PlatformStorageConnector) -> PlatformTaskTextArtifactConnector {
        PlatformTaskTextArtifactConnector::new(storage)
    }
    let _ = shape;
}
