//! Explicit single-attempt connections bound to an original observation or trusted declaration.
mod endpoint;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transport_tests;

use super::{
    inspect_platform_database_expected, verify_platform_database, PlatformCatalogExpectation,
    PlatformCatalogObservation, PlatformHostPreflightConfig,
};
pub use endpoint::PlatformDatabaseEndpoint;
use sqlx_core::connection::{ConnectOptions, Connection};
use sqlx_postgres::{PgConnectOptions, PgConnection, PgSslMode};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlatformDatabaseConnectionError {
    #[error("invalid platform database connection configuration")]
    InvalidConfig,
    #[error("ambient database startup options are not allowed")]
    AmbientStartupOptions,
    #[error("platform database connection is unavailable or unverified")]
    Unavailable,
}
type Result<T> = std::result::Result<T, PlatformDatabaseConnectionError>;

/// Trusted transport/password configuration; no Debug, Clone, serialization or options export.
pub struct PlatformDatabaseConnector {
    config: PlatformHostPreflightConfig,
    original: OriginalCatalog,
    options: PgConnectOptions,
}

enum OriginalCatalog {
    Observed(PlatformCatalogObservation),
    Expected(PlatformCatalogExpectation),
}

impl PlatformDatabaseConnector {
    /// Only the host's expected-binding factory supplies these already validated local options.
    /// Retain that same capture rather than reading any ambient startup options a second time.
    pub(super) fn from_expected_options(
        config: PlatformHostPreflightConfig,
        expectation: PlatformCatalogExpectation,
        options: PgConnectOptions,
    ) -> Result<Self> {
        if !expectation.config_matches(&config) {
            return Err(PlatformDatabaseConnectionError::InvalidConfig);
        }
        Ok(Self {
            config,
            original: OriginalCatalog::Expected(expectation),
            options,
        })
    }

    pub(super) fn binds_catalog(
        &self,
        config: &PlatformHostPreflightConfig,
        observed: &PlatformCatalogObservation,
    ) -> bool {
        if self.config != *config || !observed.config_matches(config) {
            return false;
        }
        match &self.original {
            OriginalCatalog::Observed(original) => original.matches(observed),
            OriginalCatalog::Expected(expected) => expected.matches(observed),
        }
    }

    /// Pure construction from a complete trusted declaration; no first-read identity adoption.
    /// As with `new`, transport/password trust remains the caller's responsibility.
    pub fn with_catalog_expectation(
        config: PlatformHostPreflightConfig,
        expectation: PlatformCatalogExpectation,
        endpoint: PlatformDatabaseEndpoint,
        password: &str,
    ) -> Result<Self> {
        if !expectation.config_matches(&config) {
            return Err(PlatformDatabaseConnectionError::InvalidConfig);
        }
        Ok(Self {
            options: options(&config, &endpoint, password)?,
            config,
            original: OriginalCatalog::Expected(expectation),
        })
    }

    /// Explicit local transport, password and original trusted catalog observation only.
    /// SQLx reads environment internally; inherited startup options are rejected and active
    /// coordinate/authentication values overridden. No process environment is changed.
    pub fn new(
        config: PlatformHostPreflightConfig,
        original: PlatformCatalogObservation,
        endpoint: PlatformDatabaseEndpoint,
        password: &str,
    ) -> Result<Self> {
        if !original.config_matches(&config) {
            return Err(PlatformDatabaseConnectionError::InvalidConfig);
        }
        Ok(Self {
            options: options(&config, &endpoint, password)?,
            config,
            original: OriginalCatalog::Observed(original),
        })
    }

    /// One attempt and an overall ten-second deadline through catalog verification. No pool,
    /// retry or replacement baseline. Failure/cancellation never publishes a connection.
    /// The caller-owned result is not continuously guarded against later changes.
    pub async fn connect(&self) -> Result<PgConnection> {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut connection = dial(&self.options).await?;
            match &self.original {
                OriginalCatalog::Observed(original) => {
                    verify_platform_database(&self.config, &mut connection, original).await
                }
                OriginalCatalog::Expected(expected) => {
                    inspect_platform_database_expected(&self.config, &mut connection, expected)
                        .await
                        .map(|_| ())
                }
            }
            .map_err(|_| PlatformDatabaseConnectionError::Unavailable)?;
            Ok(connection)
        })
        .await
        .map_err(|_| PlatformDatabaseConnectionError::Unavailable)?
    }
}

pub(super) fn options(
    config: &PlatformHostPreflightConfig,
    endpoint: &PlatformDatabaseEndpoint,
    password: &str,
) -> Result<PgConnectOptions> {
    if password.len() > 4096 || password.contains('\0') {
        return Err(PlatformDatabaseConnectionError::InvalidConfig);
    }
    let options = PgConnectOptions::new_without_pgpass();
    // The builder appends PGOPTIONS rather than replacing it. Reject any captured value,
    // including empty, before networking. Never parse a URL or temporarily clear global PG*.
    if options.get_options().is_some() {
        return Err(PlatformDatabaseConnectionError::AmbientStartupOptions);
    }
    Ok(endpoint
        .apply(options)
        .database(config.target().database())
        .username(config.target().role())
        .password(password)
        // Unix sockets / literal loopback only. PGSSL* inputs remain inert; this is not TLS
        // authentication or protection against handing a password to a substituted local peer.
        .ssl_mode(PgSslMode::Disable)
        .application_name("cyanrex-platform-connection")
        .statement_cache_capacity(100)
        .disable_statement_logging()
        .options([
            ("search_path", config.namespaces().source()),
            ("statement_timeout", "5000"),
            ("lock_timeout", "2000"),
            ("idle_in_transaction_session_timeout", "10000"),
        ]))
}

pub(super) async fn dial(options: &PgConnectOptions) -> Result<PgConnection> {
    tokio::time::timeout(Duration::from_secs(10), PgConnection::connect_with(options))
        .await
        .map_err(|_| PlatformDatabaseConnectionError::Unavailable)?
        .map_err(|_| PlatformDatabaseConnectionError::Unavailable)
}
