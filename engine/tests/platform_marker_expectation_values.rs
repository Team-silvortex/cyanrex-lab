use cyanrex_engine::{
    models::collaboration::WorkspaceRef,
    platform_host::{
        PlatformCatalogExpectation, PlatformDatabaseTarget, PlatformHostPreflightConfig,
        PlatformHostPreflightError, PlatformNamespaces, PlatformStorageMarkerExpectation,
    },
    services::auth_service::durable_source::SessionStockLimits,
};
use uuid::Uuid;

fn catalog() -> PlatformCatalogExpectation {
    let config = PlatformHostPreflightConfig::new(
        "https://platform.example",
        WorkspaceRef {
            authority_id: Uuid::from_u128(4201).try_into().unwrap(),
            workspace_id: Uuid::from_u128(4202).try_into().unwrap(),
        },
        PlatformNamespaces::new("source_private", "task_private", "artifacts_private").unwrap(),
        PlatformDatabaseTarget::new("platform_db", "platform_runtime", 123, 456).unwrap(),
        SessionStockLimits::new(2, 8).unwrap(),
    )
    .unwrap();
    PlatformCatalogExpectation::new(config, 11, 12, [21, 22, 23], [12; 3]).unwrap()
}

#[test]
fn platform_marker_expectation_accepts_complete_ordered_declarations_without_io() {
    for (oids, owners) in [
        ([101, 102, 103, 104, 105], [12; 5]),
        (
            [
                u32::MAX - 4,
                u32::MAX - 3,
                u32::MAX - 2,
                u32::MAX - 1,
                u32::MAX,
            ],
            [u32::MAX; 5],
        ),
    ] {
        assert!(PlatformStorageMarkerExpectation::new(catalog(), oids, owners).is_ok());
    }
}

#[test]
fn platform_marker_expectation_rejects_zero_duplicate_or_runtime_owned_identities() {
    for index in 0..5 {
        let mut oids = [101, 102, 103, 104, 105];
        oids[index] = 0;
        assert_eq!(
            PlatformStorageMarkerExpectation::new(catalog(), oids, [12; 5]).map(|_| ()),
            Err(PlatformHostPreflightError::InvalidConfig)
        );
        let mut owners = [12; 5];
        owners[index] = 0;
        assert_eq!(
            PlatformStorageMarkerExpectation::new(catalog(), [101, 102, 103, 104, 105], owners)
                .map(|_| ()),
            Err(PlatformHostPreflightError::InvalidConfig)
        );
        owners[index] = 11;
        assert_eq!(
            PlatformStorageMarkerExpectation::new(catalog(), [101, 102, 103, 104, 105], owners)
                .map(|_| ()),
            Err(PlatformHostPreflightError::InvalidConfig)
        );
    }
    for left in 0..5 {
        for right in left + 1..5 {
            let mut oids = [101, 102, 103, 104, 105];
            oids[right] = oids[left];
            assert_eq!(
                PlatformStorageMarkerExpectation::new(catalog(), oids, [12; 5]).map(|_| ()),
                Err(PlatformHostPreflightError::InvalidConfig)
            );
        }
    }
}

#[test]
fn platform_marker_expectation_allows_shared_owners_and_values_from_different_catalogs() {
    // pg_class, pg_namespace and pg_authid have separate OID domains. Only the five
    // marker relation identities must be distinct from each other.
    assert!(
        PlatformStorageMarkerExpectation::new(catalog(), [11, 12, 21, 22, 23], [12; 5]).is_ok()
    );
    assert!(PlatformStorageMarkerExpectation::new(
        catalog(),
        [101, 102, 103, 104, 105],
        [21, 22, 23, 21, 22]
    )
    .is_ok());
}
