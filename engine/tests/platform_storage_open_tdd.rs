#![cfg(target_os = "linux")]
//! Explicit expected-binding preparation, never a listener, migration or baseline discovery.
use cyanrex_engine::platform_host::{
    open_platform_storage_expected, PlatformArtifactRootObservation, PlatformDatabaseEndpoint,
    PlatformHostPreflightConfig, PlatformStorageConnectionError, PlatformStorageConnector,
    PlatformStorageMarkerExpectation,
};
use std::path::Path;

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
#[path = "platform_storage_open/admission.rs"]
mod admission;
#[path = "platform_storage_open/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "platform_host_preflight/fixture.rs"]
mod catalog_fixture;
#[allow(dead_code)]
#[path = "platform_catalog_expectation/support.rs"]
mod expectation_helpers;
#[path = "platform_storage_open/fixture.rs"]
mod fixture;
#[path = "platform_storage_open/identity.rs"]
mod identity;
#[allow(dead_code)]
#[path = "platform_login_connection/fixture.rs"]
mod login_fixture;
#[allow(dead_code)]
#[path = "platform_storage_markers/fixture.rs"]
mod marker_fixture;
#[allow(dead_code)]
#[path = "platform_marker_expectation/support.rs"]
mod marker_helpers;
#[allow(dead_code)]
#[path = "platform_storage_connection/support.rs"]
mod storage_helpers;
#[allow(dead_code)]
#[path = "platform_storage_check/support.rs"]
mod wait_helpers;
#[path = "platform_storage_open/waits.rs"]
mod waits;

async fn open_result(
    config: PlatformHostPreflightConfig,
    endpoint: PlatformDatabaseEndpoint,
    expected: PlatformStorageMarkerExpectation,
    root: &Path,
    original: PlatformArtifactRootObservation,
) -> Result<PlatformStorageConnector, PlatformStorageConnectionError> {
    open_platform_storage_expected(
        config,
        endpoint,
        "synthetic-password",
        expected,
        root,
        original,
    )
    .await
}

#[test]
fn platform_storage_open_retains_an_owned_explicit_declaration_without_exporting_observations() {
    let _ = open_result;
    fn traits<T: Send + Sync>() {}
    traits::<PlatformStorageConnector>();
}
