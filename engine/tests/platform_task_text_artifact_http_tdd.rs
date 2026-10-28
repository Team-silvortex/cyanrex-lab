#![cfg(unix)]
//! Explicit standalone Task-bound text publication HTTP; no host/browser/SDK mount.
use cyanrex_engine::{
    platform_host::PlatformTaskTextArtifactConnector,
    platform_http::{build_task_text_artifact_publish_router, TaskTextArtifactPublishHttpState},
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
#[path = "platform_task_text_artifact_http/admission.rs"]
mod admission;
#[path = "platform_task_text_artifact_http/authority.rs"]
mod authority;
#[path = "platform_task_text_artifact_http/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code, unused_imports)]
#[path = "platform_task_content_connection/fixture.rs"]
mod content_fixture;
#[path = "platform_task_text_artifact_http/drift.rs"]
mod drift;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_task_text_artifact_http/faults.rs"]
mod faults;
#[path = "platform_task_text_artifact_http/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_task_index_connection/fixture.rs"]
mod index_fixture;
#[path = "platform_task_text_artifact_http/ingress.rs"]
mod ingress;
#[allow(dead_code)]
#[path = "platform_text_artifact_connection/fixture.rs"]
mod retained_fixture;
#[path = "platform_task_index_connection/schema.rs"]
mod schema;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
#[allow(dead_code)]
#[path = "platform_task_text_artifact_connection/fixture.rs"]
mod task_bound_fixture;

#[test]
fn task_text_artifact_http_requires_an_explicit_task_bound_connector() {
    fn shape(connector: PlatformTaskTextArtifactConnector) -> axum::Router {
        build_task_text_artifact_publish_router(
            TaskTextArtifactPublishHttpState::new(connector).unwrap(),
        )
    }
    let _ = shape;
}
