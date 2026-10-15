#![cfg(unix)]

use cyanrex_engine::{
    models::collaboration::WorkspaceRef,
    platform_host::{
        PlatformCatalogExpectation, PlatformDatabaseTarget, PlatformHostPreflightConfig,
        PlatformNamespaces,
    },
    services::auth_service::durable_source::SessionStockLimits,
};
use uuid::Uuid;

fn config() -> PlatformHostPreflightConfig {
    PlatformHostPreflightConfig::new(
        "https://platform.example:8443",
        WorkspaceRef {
            authority_id: Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: Uuid::from_u128(4202).try_into().unwrap(),
        },
        PlatformNamespaces::new("source_private", "task_private", "artifacts_private").unwrap(),
        PlatformDatabaseTarget::new("platform_db", "platform_runtime", 123, 456).unwrap(),
        SessionStockLimits::new(4, 16).unwrap(),
    )
    .unwrap()
}

#[test]
fn platform_catalog_expectation_accepts_explicit_nonzero_distinct_namespaces_and_shared_owners() {
    assert!(PlatformCatalogExpectation::new(config(), 101, 102, [201, 202, 203], [102; 3]).is_ok());
    assert!(
        PlatformCatalogExpectation::new(config(), 101, 102, [201, 202, 203], [103, 104, 105])
            .is_ok()
    );
    assert!(PlatformCatalogExpectation::new(
        config(),
        1,
        u32::MAX,
        [u32::MAX, 1, 2],
        [u32::MAX; 3]
    )
    .is_ok());
}

#[test]
fn platform_catalog_expectation_rejects_zero_in_every_oid_position() {
    assert!(PlatformCatalogExpectation::new(config(), 0, 102, [201, 202, 203], [102; 3]).is_err());
    assert!(PlatformCatalogExpectation::new(config(), 101, 0, [201, 202, 203], [102; 3]).is_err());
    for index in 0..3 {
        let mut namespaces = [201, 202, 203];
        namespaces[index] = 0;
        assert!(PlatformCatalogExpectation::new(config(), 101, 102, namespaces, [102; 3]).is_err());
        let mut owners = [102; 3];
        owners[index] = 0;
        assert!(
            PlatformCatalogExpectation::new(config(), 101, 102, [201, 202, 203], owners).is_err()
        );
    }
}

#[test]
fn platform_catalog_expectation_rejects_all_namespace_oid_aliases() {
    for (left, right) in [(0, 1), (0, 2), (1, 2)] {
        let mut namespaces = [201, 202, 203];
        namespaces[right] = namespaces[left];
        assert!(PlatformCatalogExpectation::new(config(), 101, 102, namespaces, [102; 3]).is_err());
    }
    assert!(PlatformCatalogExpectation::new(config(), 101, 102, [201; 3], [102; 3]).is_err());
}

#[test]
fn platform_catalog_expectation_rejects_runtime_role_owning_database_or_any_namespace() {
    assert!(
        PlatformCatalogExpectation::new(config(), 101, 101, [201, 202, 203], [102; 3]).is_err()
    );
    for index in 0..3 {
        let mut owners = [102; 3];
        owners[index] = 101;
        assert!(
            PlatformCatalogExpectation::new(config(), 101, 102, [201, 202, 203], owners).is_err()
        );
    }
}

#[test]
fn platform_catalog_expectation_allows_numeric_oid_overlap_between_different_catalogs() {
    // Namespace/role/database catalogs have independent OID domains; ownership still uses roles.
    assert!(PlatformCatalogExpectation::new(config(), 7, 8, [7, 8, 456], [8; 3]).is_ok());
    assert!(
        PlatformCatalogExpectation::new(config(), 456, 201, [201, 202, 203], [201, 202, 203])
            .is_ok()
    );
    assert!(
        PlatformCatalogExpectation::new(config(), u32::MAX, 1, [u32::MAX, 1, 456], [1; 3]).is_ok()
    );
}
