#![cfg(unix)]

// Only the old resource owner is reused; none of its previous test cases are nested here.
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;

mod config {
    pub(super) fn scope() -> cyanrex_engine::models::collaboration::WorkspaceRef {
        cyanrex_engine::models::collaboration::WorkspaceRef {
            authority_id: uuid::Uuid::from_u128(8201).try_into().unwrap(),
            workspace_id: uuid::Uuid::from_u128(8202).try_into().unwrap(),
        }
    }
}

#[path = "platform_storage_markers/behavior.rs"]
mod behavior;
#[path = "platform_storage_markers/control.rs"]
mod control;
#[path = "platform_storage_markers/fixture.rs"]
mod fixture;
#[path = "platform_storage_markers/pins.rs"]
mod pins;
#[path = "platform_storage_markers/rejections.rs"]
mod rejections;
#[path = "platform_storage_markers/shapes.rs"]
mod shapes;
#[path = "platform_storage_markers/support.rs"]
mod support;
