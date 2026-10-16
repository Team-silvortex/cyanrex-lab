use super::*;
use crate::platform_host::PlatformCatalogExpectation;

/// Complete trusted catalog/marker assertion, not an observation or authorization.
pub struct PlatformStorageMarkerExpectation {
    pub(super) catalog: PlatformCatalogExpectation,
    pub(super) markers: [MarkerIdentity; 5],
}

impl PlatformStorageMarkerExpectation {
    /// Fixed order: auth Source, identity, access, Task, Artifact schema markers.
    pub fn new(
        catalog: PlatformCatalogExpectation,
        marker_oids: [u32; 5],
        marker_owner_oids: [u32; 5],
    ) -> Result<Self> {
        if marker_oids.contains(&0)
            || marker_owner_oids.contains(&0)
            || marker_owner_oids.contains(&catalog.role_oid())
            || marker_oids
                .iter()
                .enumerate()
                .any(|(index, oid)| marker_oids[..index].contains(oid))
        {
            return Err(PlatformHostPreflightError::InvalidConfig);
        }
        Ok(Self {
            catalog,
            markers: std::array::from_fn(|index| MarkerIdentity {
                oid: marker_oids[index],
                owner_oid: marker_owner_oids[index],
            }),
        })
    }

    pub(in crate::platform_host) fn config_matches(
        &self,
        config: &PlatformHostPreflightConfig,
    ) -> bool {
        self.catalog.config_matches(config)
    }
}

/// One owned read-only transaction verifies all declared identities before marker row reads.
/// A declaration never becomes an observation until real catalog, shape and scope checks commit.
pub async fn inspect_platform_storage_markers_expected(
    config: &PlatformHostPreflightConfig,
    connection: &mut PgConnection,
    expected: &PlatformStorageMarkerExpectation,
) -> Result<PlatformStorageMarkerObservation> {
    inspect_bound(
        config,
        connection,
        CatalogBinding::Declared(&expected.catalog),
        Some(&expected.markers),
    )
    .await
}
