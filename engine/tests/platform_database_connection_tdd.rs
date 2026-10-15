#![cfg(unix)]

// Reuse only the owned, panic-safe database/role fixture. No existing test module is
// imported, so this target has its own explicit opt-in inventory.
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

#[path = "platform_database_connection/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[path = "platform_database_connection/helpers.rs"]
mod helpers;
#[path = "platform_database_connection/identity.rs"]
mod identity;
#[path = "platform_database_connection/privileges.rs"]
mod privileges;
