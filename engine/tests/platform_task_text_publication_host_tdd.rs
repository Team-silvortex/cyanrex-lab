#![cfg(target_os = "linux")]
//! Explicit Task-bound opt-in workspace and capless children, not browser or LAN acceptance.
use cyanrex_engine::{
    platform_host::PlatformStorageConnector,
    platform_http::{PlatformWorkspaceHttpState, TaskContentHttpConfigError},
};

mod config {
    use cyanrex_engine::models::collaboration::WorkspaceRef;
    pub(super) fn scope() -> WorkspaceRef {
        WorkspaceRef {
            authority_id: uuid::Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: uuid::Uuid::from_u128(4202).try_into().unwrap(),
        }
    }
}
#[path = "platform_task_text_publication_host/boundaries.rs"]
mod boundaries;
#[path = "platform_task_text_publication_host/capacity.rs"]
mod capacity;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code)]
#[path = "platform_serve_loopback_tls_defaults/certificates.rs"]
mod certificates;
#[allow(dead_code)]
#[path = "platform_loopback_host/child.rs"]
mod child;
#[allow(dead_code, unused_imports)]
#[path = "platform_task_content_connection/fixture.rs"]
mod content_fixture;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_task_text_publication_host/faults.rs"]
mod faults;
#[path = "platform_task_text_publication_host/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_task_index_connection/fixture.rs"]
mod index_fixture;
#[allow(dead_code)]
#[path = "platform_config_input/kernel.rs"]
mod kernel;
#[allow(dead_code)]
#[path = "platform_marker_expectation/support.rs"]
mod marker_helpers;
#[path = "platform_task_text_publication_host/native.rs"]
mod native;
#[path = "platform_task_text_publication_host/native_fixture.rs"]
mod native_fixture;
#[allow(dead_code, unused_imports)]
#[path = "platform_text_publication_host/fixture.rs"]
mod publication_fixture;
#[allow(dead_code, unused_imports)]
#[path = "platform_text_artifact_http/fixture.rs"]
mod publication_http;
#[allow(dead_code)]
#[path = "platform_text_artifact_connection/fixture.rs"]
mod retained_fixture;
#[path = "platform_task_index_connection/schema.rs"]
mod schema;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
#[path = "platform_task_text_publication_host/workspace.rs"]
mod workspace;

#[test]
fn task_text_publication_workspace_requires_an_explicit_constructor() {
    fn shape(
        storage: PlatformStorageConnector,
    ) -> Result<PlatformWorkspaceHttpState, TaskContentHttpConfigError> {
        PlatformWorkspaceHttpState::with_task_text_publication_capacities(
            storage, 1, 1, 1, 1, 1, 1, 1, 1,
        )
    }
    let _ = shape;
}
