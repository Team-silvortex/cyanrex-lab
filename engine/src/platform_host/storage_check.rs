//! One-shot read-only diagnostics, never an original runtime baseline or connection handoff.
use super::{
    database_connection::{dial, options},
    inspect_platform_database, inspect_platform_database_expected,
    inspect_platform_storage_markers, PlatformCatalogExpectation, PlatformDatabaseEndpoint,
    PlatformHostPreflightConfig, PlatformHostPreflightError as Error, Result,
};
use sqlx_core::connection::Connection;
use std::time::Duration;

/// One fresh explicit connection; observe configured physical target/role/namespaces and five
/// current version/scope markers, then close. No observation or connection is returned or retained.
/// The trusted caller owns process preparation and local-peer trust before password delivery.
/// This is not historical identity verification, complete schema health or Session authorization.
pub async fn check_platform_database_storage(
    config: &PlatformHostPreflightConfig,
    endpoint: &PlatformDatabaseEndpoint,
    password: &str,
) -> Result<()> {
    check(config, endpoint, password, None).await
}

/// The same one-shot diagnostic, but compare against an explicit trusted catalog declaration
/// before inspecting current markers. This does not pin historical marker or Artifact root IDs.
pub async fn check_platform_database_storage_expected(
    config: &PlatformHostPreflightConfig,
    endpoint: &PlatformDatabaseEndpoint,
    password: &str,
    expected: &PlatformCatalogExpectation,
) -> Result<()> {
    check(config, endpoint, password, Some(expected)).await
}

async fn check(
    config: &PlatformHostPreflightConfig,
    endpoint: &PlatformDatabaseEndpoint,
    password: &str,
    expected: Option<&PlatformCatalogExpectation>,
) -> Result<()> {
    if expected.is_some_and(|expected| !expected.config_matches(config)) {
        return Err(Error::CatalogChanged);
    }
    // Options reject captured PGOPTIONS before any network call, without changing global state.
    let options = options(config, endpoint, password).map_err(|_| Error::InvalidConfig)?;
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut connection = dial(&options)
            .await
            .map_err(|_| Error::StorageUnavailable)?;
        let inspected = async {
            let catalog = match expected {
                Some(expected) => {
                    inspect_platform_database_expected(config, &mut connection, expected).await?
                }
                None => inspect_platform_database(config, &mut connection).await?,
            };
            inspect_platform_storage_markers(config, &mut connection, &catalog).await?;
            Ok(())
        }
        .await;
        // Close after either inspection outcome, within the same total database deadline.
        // Timeout/cancellation instead drops unpublished state; it is not remote rollback proof.
        let closed = connection
            .close()
            .await
            .map_err(|_| Error::StorageUnavailable);
        inspected.and(closed)
    })
    .await
    .map_err(|_| Error::StorageUnavailable)?
}
