//! Private explicit input. No options, connection, baseline observation or secret reporting.
use cyanrex_engine::{
    models::collaboration::{AuthorityId, WorkspaceId, WorkspaceRef},
    platform_host::{
        PlatformCatalogExpectation, PlatformDatabaseEndpoint, PlatformDatabaseTarget,
        PlatformHostPreflightConfig, PlatformNamespaces,
    },
    services::auth_service::durable_source::SessionStockLimits,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

mod map_only;
use map_only::MapOnly;
mod expected_catalog;
use expected_catalog::ExpectedField;
#[cfg(test)]
mod expectation_tests;
#[cfg(test)]
mod tests;

// Every layer is strict and required. Never derive Debug, Clone or Serialize for this input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    format_version: u8,
    origin: String,
    authority_id: AuthorityId,
    workspace_id: WorkspaceId,
    namespaces: MapOnly<Namespaces>,
    database: MapOnly<Database>,
    session_limits: MapOnly<Limits>,
    artifact_root: String,
    #[serde(default)]
    catalog_identity: ExpectedField,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Namespaces {
    source: String,
    task: String,
    artifacts: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Database {
    database: String,
    role: String,
    system_identifier: u64,
    database_oid: u32,
    transport: MapOnly<Transport>,
    port: u16,
    password: String,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Transport {
    Unix { directory: PathBuf },
    Loopback { address: std::net::IpAddr },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Limits {
    per_account: u16,
    total: u16,
}

// Validated only, not an observed target or authority. Never Debug/Clone/Serialize.
pub(super) struct ValidatedInput {
    pub(super) config: PlatformHostPreflightConfig,
    pub(super) endpoint: PlatformDatabaseEndpoint,
    pub(super) password: String,
    pub(super) artifact_root: PathBuf,
    pub(super) catalog_expectation: Option<PlatformCatalogExpectation>,
}

pub(super) fn check(path: &Path) -> Result<(), ()> {
    validate(&super::private_input::read(path)?)
}

fn validate(bytes: &[u8]) -> Result<(), ()> {
    parse(bytes).map(|_| ())
}

pub(super) fn load(path: &Path) -> Result<ValidatedInput, ()> {
    parse(&super::private_input::read(path)?)
}

fn parse(bytes: &[u8]) -> Result<ValidatedInput, ()> {
    if bytes.len() > 16 * 1024 {
        return Err(());
    }
    let input = serde_json::from_slice::<MapOnly<Input>>(bytes)
        .map_err(|_| ())?
        .0;
    if (input.format_version != 1 && input.format_version != 2)
        || input.database.0.password.len() > 4096
        || input.database.0.password.contains('\0')
        || !canonical_path(&input.artifact_root)
    {
        return Err(());
    }
    let database = input.database.0;
    let endpoint = match database.transport.0 {
        Transport::Unix { directory } => PlatformDatabaseEndpoint::unix(&directory, database.port),
        Transport::Loopback { address } => {
            PlatformDatabaseEndpoint::loopback(address, database.port)
        }
    }
    .map_err(|_| ())?;
    let config = PlatformHostPreflightConfig::new(
        &input.origin,
        WorkspaceRef {
            authority_id: input.authority_id,
            workspace_id: input.workspace_id,
        },
        PlatformNamespaces::new(
            &input.namespaces.0.source,
            &input.namespaces.0.task,
            &input.namespaces.0.artifacts,
        )
        .map_err(|_| ())?,
        PlatformDatabaseTarget::new(
            &database.database,
            &database.role,
            database.system_identifier,
            database.database_oid,
        )
        .map_err(|_| ())?,
        SessionStockLimits::new(
            input.session_limits.0.per_account,
            input.session_limits.0.total,
        )
        .map_err(|_| ())?,
    )
    .map_err(|_| ())?;
    let catalog_expectation = input.catalog_identity.bind(input.format_version, &config)?;
    // The check-only caller discards this value; explicit storage diagnostics consume it once.
    // Drop is not secure zeroization. Parsing opens no target or Artifact path.
    Ok(ValidatedInput {
        config,
        endpoint,
        password: database.password,
        artifact_root: PathBuf::from(input.artifact_root),
        catalog_expectation,
    })
}

pub(super) fn canonical_path(text: &str) -> bool {
    text.len() <= 4096
        && text.starts_with('/')
        && !text.chars().any(char::is_control)
        && !text
            .split('/')
            .skip(1)
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
}
