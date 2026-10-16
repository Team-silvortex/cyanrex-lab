//! One-shot read-only diagnostics, never an original runtime baseline or connection handoff.
use super::{
    database_connection::{dial, options},
    inspect_platform_database, inspect_platform_database_expected,
    inspect_platform_storage_markers, inspect_platform_storage_markers_expected,
    PlatformCatalogExpectation, PlatformDatabaseEndpoint, PlatformHostPreflightConfig,
    PlatformHostPreflightError as Error, PlatformStorageMarkerExpectation, Result,
};
use sqlx_core::connection::Connection;
use sqlx_postgres::PgConnection;
use std::time::Duration;

enum Expected<'a> {
    Current,
    Catalog(&'a PlatformCatalogExpectation),
    Markers(&'a PlatformStorageMarkerExpectation),
}

impl Expected<'_> {
    fn config_matches(&self, config: &PlatformHostPreflightConfig) -> bool {
        match self {
            Self::Current => true,
            Self::Catalog(expected) => expected.config_matches(config),
            Self::Markers(expected) => expected.config_matches(config),
        }
    }

    async fn inspect(
        self,
        config: &PlatformHostPreflightConfig,
        connection: &mut PgConnection,
    ) -> Result<()> {
        let catalog = match self {
            Self::Current => inspect_platform_database(config, connection).await?,
            Self::Catalog(expected) => {
                inspect_platform_database_expected(config, connection, expected).await?
            }
            Self::Markers(expected) => {
                return inspect_platform_storage_markers_expected(config, connection, expected)
                    .await
                    .map(|_| ())
            }
        };
        inspect_platform_storage_markers(config, connection, &catalog).await?;
        Ok(())
    }
}

/// One-shot original catalog AND five-marker declaration check, not a retained runtime baseline.
pub async fn check_platform_database_storage_markers_expected(
    config: &PlatformHostPreflightConfig,
    endpoint: &PlatformDatabaseEndpoint,
    password: &str,
    expected: &PlatformStorageMarkerExpectation,
) -> Result<()> {
    check(config, endpoint, password, Expected::Markers(expected)).await
}

/// One fresh explicit connection; observe configured physical target/role/namespaces and five
/// current version/scope markers, then close. No observation or connection is returned or retained.
/// The trusted caller owns process preparation and local-peer trust before password delivery.
/// This is not historical identity verification, complete schema health or Session authorization.
pub async fn check_platform_database_storage(
    config: &PlatformHostPreflightConfig,
    endpoint: &PlatformDatabaseEndpoint,
    password: &str,
) -> Result<()> {
    check(config, endpoint, password, Expected::Current).await
}

/// The same one-shot diagnostic, but compare against an explicit trusted catalog declaration
/// before inspecting current markers. This does not pin historical marker or Artifact root IDs.
pub async fn check_platform_database_storage_expected(
    config: &PlatformHostPreflightConfig,
    endpoint: &PlatformDatabaseEndpoint,
    password: &str,
    expected: &PlatformCatalogExpectation,
) -> Result<()> {
    check(config, endpoint, password, Expected::Catalog(expected)).await
}

async fn check(
    config: &PlatformHostPreflightConfig,
    endpoint: &PlatformDatabaseEndpoint,
    password: &str,
    expected: Expected<'_>,
) -> Result<()> {
    if !expected.config_matches(config) {
        return Err(Error::CatalogChanged);
    }
    // Options reject captured PGOPTIONS before any network call, without changing global state.
    let options = options(config, endpoint, password).map_err(|_| Error::InvalidConfig)?;
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut connection = dial(&options)
            .await
            .map_err(|_| Error::StorageUnavailable)?;
        let inspected = expected.inspect(config, &mut connection).await;
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
