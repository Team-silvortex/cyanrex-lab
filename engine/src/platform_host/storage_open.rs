//! Explicit complete assertion to retained binding, never first-read adoption or diagnostics.
use super::{
    database_connection::{dial, options},
    inspect_platform_storage_markers_expected, verify_platform_artifact_root,
    PlatformArtifactRootObservation, PlatformDatabaseConnector, PlatformDatabaseEndpoint,
    PlatformHostPreflightConfig, PlatformStorageConnectionError as Error, PlatformStorageConnector,
    PlatformStorageMarkerExpectation,
};
use sqlx_core::connection::Connection;
use std::{path::Path, time::Duration};

/// One complete trusted declaration plus a caller-held original root, never current-ID discovery.
/// The caller prepares the process and trusts the local peer before runtime/password delivery.
/// Root metadata is synchronous and outside the database's total ten-second waiting budget.
/// Cancellation publishes no binding and does not prove a remote rollback or final root recheck.
pub async fn open_platform_storage_expected(
    config: PlatformHostPreflightConfig,
    endpoint: PlatformDatabaseEndpoint,
    password: &str,
    expected: PlatformStorageMarkerExpectation,
    root_path: &Path,
    original_root: PlatformArtifactRootObservation,
) -> Result<PlatformStorageConnector, Error> {
    // Passive exact comparisons precede filesystem access, ambient options or database work.
    if !expected.config_matches(&config) || !original_root.binds_path(&config, root_path) {
        return Err(Error::InvalidConfig);
    }
    verify_platform_artifact_root(&config, root_path, &original_root)
        .map_err(|_| Error::Unavailable)?;
    // Capture once. The returned connector retains these same options, not a later PG* snapshot.
    let options = options(&config, &endpoint, password).map_err(|_| Error::InvalidConfig)?;
    let inspected = tokio::time::timeout(Duration::from_secs(10), async {
        let mut connection = dial(&options).await.map_err(|_| Error::Unavailable)?;
        let inspected =
            inspect_platform_storage_markers_expected(&config, &mut connection, &expected)
                .await
                .map_err(|_| Error::Unavailable);
        // Close even after failed inspection, sharing the original total database deadline.
        // A close failure or timeout never publishes even a matching observed baseline.
        let closed = connection.close().await.map_err(|_| Error::Unavailable);
        match (inspected, closed) {
            (Ok(observation), Ok(())) => Ok(observation),
            _ => Err(Error::Unavailable),
        }
    })
    .await
    .unwrap_or(Err(Error::Unavailable));
    // A failed database attempt still rechecks the same original root. No replacement is adopted.
    let root = verify_platform_artifact_root(&config, root_path, &original_root)
        .map_err(|_| Error::Unavailable);
    let markers = inspected?;
    root?;
    let database = PlatformDatabaseConnector::from_expected_options(
        config.clone(),
        expected.into_catalog(),
        options,
    )
    .map_err(|_| Error::InvalidConfig)?;
    PlatformStorageConnector::new(config, database, markers, root_path, original_root)
}
