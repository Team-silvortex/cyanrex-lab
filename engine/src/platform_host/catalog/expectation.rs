use super::{
    inspect_platform_database, NamespaceIdentity, PlatformCatalogObservation,
    PlatformHostPreflightConfig, PlatformHostPreflightError, Result,
};
use sqlx_postgres::PgConnection;

/// An explicit trusted caller assertion, not an observation, permission or health receipt.
/// No current database values are adopted to fill missing identities or replace this declaration.
pub struct PlatformCatalogExpectation {
    config: PlatformHostPreflightConfig,
    role_oid: u32,
    database_owner_oid: u32,
    namespaces: [NamespaceIdentity; 3],
}

impl PlatformCatalogExpectation {
    pub(in crate::platform_host) fn role_oid(&self) -> u32 {
        self.role_oid
    }

    /// Namespace OIDs and their owner OIDs are ordered Source, Task, Artifact.
    pub fn new(
        config: PlatformHostPreflightConfig,
        role_oid: u32,
        database_owner_oid: u32,
        namespace_oids: [u32; 3],
        namespace_owner_oids: [u32; 3],
    ) -> Result<Self> {
        // Namespace OIDs are distinct within pg_namespace, not across independent catalogs.
        // Owners may be shared, but the low-privilege runtime role cannot own these objects.
        if role_oid == 0
            || database_owner_oid == 0
            || role_oid == database_owner_oid
            || namespace_oids.contains(&0)
            || namespace_owner_oids.contains(&0)
            || namespace_owner_oids.contains(&role_oid)
            || namespace_oids[0] == namespace_oids[1]
            || namespace_oids[0] == namespace_oids[2]
            || namespace_oids[1] == namespace_oids[2]
        {
            return Err(PlatformHostPreflightError::InvalidConfig);
        }
        Ok(Self {
            config,
            role_oid,
            database_owner_oid,
            namespaces: std::array::from_fn(|index| NamespaceIdentity {
                oid: namespace_oids[index],
                owner_oid: namespace_owner_oids[index],
            }),
        })
    }

    pub(in crate::platform_host) fn config_matches(
        &self,
        config: &PlatformHostPreflightConfig,
    ) -> bool {
        &self.config == config
    }

    pub(in crate::platform_host) fn matches(&self, observed: &PlatformCatalogObservation) -> bool {
        self.config == observed.config
            && self.role_oid == observed.role_oid
            && self.database_owner_oid == observed.database_owner_oid
            && self.namespaces == observed.namespaces
    }
}

/// Observe on an idle borrowed connection and compare with the complete trusted declaration.
/// Config mismatch is rejected before SQL. Only a genuine matching observation is returned.
pub async fn inspect_platform_database_expected(
    config: &PlatformHostPreflightConfig,
    connection: &mut PgConnection,
    expected: &PlatformCatalogExpectation,
) -> Result<PlatformCatalogObservation> {
    if !expected.config_matches(config) {
        return Err(PlatformHostPreflightError::CatalogChanged);
    }
    let observed = inspect_platform_database(config, connection).await?;
    if !expected.matches(&observed) {
        return Err(PlatformHostPreflightError::CatalogChanged);
    }
    Ok(observed)
}
