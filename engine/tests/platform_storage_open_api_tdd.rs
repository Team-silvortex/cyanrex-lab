#![cfg(unix)]
//! Compile-time ownership contract, not storage availability or successful runtime preparation.
use cyanrex_engine::platform_host::{
    open_platform_storage_expected, PlatformArtifactRootObservation, PlatformDatabaseEndpoint,
    PlatformHostPreflightConfig, PlatformStorageConnectionError, PlatformStorageConnector,
    PlatformStorageMarkerExpectation,
};
use std::path::Path;

async fn retained_binding(
    config: PlatformHostPreflightConfig,
    endpoint: PlatformDatabaseEndpoint,
    expected: PlatformStorageMarkerExpectation,
    root_path: &Path,
    original_root: PlatformArtifactRootObservation,
) -> Result<PlatformStorageConnector, PlatformStorageConnectionError> {
    open_platform_storage_expected(
        config,
        endpoint,
        "public-synthetic-password",
        expected,
        root_path,
        original_root,
    )
    .await
}

#[test]
fn platform_storage_open_consumes_one_complete_assertion_and_the_original_root() {
    let _ = retained_binding;
    fn send_sync<T: Send + Sync>() {}
    send_sync::<PlatformStorageConnector>();
}
