use super::{PlatformHostPreflightError, Result};
use crate::{
    models::collaboration::WorkspaceRef,
    platform_origin,
    services::auth_service::durable_source::{valid_platform_namespace, SessionStockLimits},
};

/// Operator-supplied physical database coordinates, not a connection string or secret.
#[derive(Clone, PartialEq, Eq)]
pub struct PlatformDatabaseTarget {
    database: String,
    role: String,
    system_identifier: u64,
    database_oid: u32,
}

impl PlatformDatabaseTarget {
    pub fn new(
        database: &str,
        role: &str,
        system_identifier: u64,
        database_oid: u32,
    ) -> Result<Self> {
        let valid_identifier = |value: &str| {
            !value.is_empty()
                && value.len() <= 63
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        };
        if !valid_identifier(database)
            || !valid_identifier(role)
            || system_identifier == 0
            || database_oid == 0
        {
            return Err(PlatformHostPreflightError::InvalidConfig);
        }
        Ok(Self {
            database: database.to_owned(),
            role: role.to_owned(),
            system_identifier,
            database_oid,
        })
    }

    pub fn database(&self) -> &str {
        &self.database
    }
    pub fn role(&self) -> &str {
        &self.role
    }
    pub fn system_identifier(&self) -> u64 {
        self.system_identifier
    }
    pub fn database_oid(&self) -> u32 {
        self.database_oid
    }
}

/// Three exact, distinct private namespaces. The source namespace also contains its registry.
#[derive(Clone, PartialEq, Eq)]
pub struct PlatformNamespaces {
    source: String,
    task: String,
    artifacts: String,
}

impl PlatformNamespaces {
    pub fn new(source: &str, task: &str, artifacts: &str) -> Result<Self> {
        if ![source, task, artifacts]
            .into_iter()
            .all(valid_platform_namespace)
            || source == task
            || source == artifacts
            || task == artifacts
        {
            return Err(PlatformHostPreflightError::InvalidNamespace);
        }
        Ok(Self {
            source: source.to_owned(),
            task: task.to_owned(),
            artifacts: artifacts.to_owned(),
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn task(&self) -> &str {
        &self.task
    }
    pub fn artifacts(&self) -> &str {
        &self.artifacts
    }
}

/// Pure caller-owned preparation. A valid value neither opens storage nor proves its health.
#[derive(Clone, PartialEq, Eq)]
pub struct PlatformHostPreflightConfig {
    origin: String,
    scope: WorkspaceRef,
    namespaces: PlatformNamespaces,
    target: PlatformDatabaseTarget,
    limits: SessionStockLimits,
}

impl PlatformHostPreflightConfig {
    pub fn new(
        origin: &str,
        scope: WorkspaceRef,
        namespaces: PlatformNamespaces,
        target: PlatformDatabaseTarget,
        limits: SessionStockLimits,
    ) -> Result<Self> {
        platform_origin::validate(origin).map_err(|_| PlatformHostPreflightError::InvalidOrigin)?;
        Ok(Self {
            origin: origin.to_owned(),
            scope,
            namespaces,
            target,
            limits,
        })
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn scope(&self) -> WorkspaceRef {
        self.scope
    }
    pub fn namespaces(&self) -> &PlatformNamespaces {
        &self.namespaces
    }
    pub fn target(&self) -> &PlatformDatabaseTarget {
        &self.target
    }
    pub fn limits(&self) -> SessionStockLimits {
        self.limits
    }
}
