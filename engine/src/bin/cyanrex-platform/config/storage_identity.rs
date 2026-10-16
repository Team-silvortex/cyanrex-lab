//! One validated diagnostic mode; no overlapping optional declarations or database dispatch.
use super::{CatalogIdentity, MapOnly, MarkerIdentity, Presence};
use cyanrex_engine::platform_host::{
    PlatformCatalogExpectation, PlatformHostPreflightConfig, PlatformStorageMarkerExpectation,
};

pub(crate) enum StorageExpectation {
    Current,
    Catalog(PlatformCatalogExpectation),
    Markers(PlatformStorageMarkerExpectation),
}

impl StorageExpectation {
    pub(super) fn bind(
        version: u8,
        catalog: Presence<CatalogIdentity>,
        markers: Presence<MarkerIdentity>,
        config: &PlatformHostPreflightConfig,
    ) -> Result<Self, ()> {
        match (version, catalog, markers) {
            (1, Presence::Absent, Presence::Absent) => Ok(Self::Current),
            (2, Presence::Present(MapOnly(catalog)), Presence::Absent) => {
                catalog.bind(config).map(Self::Catalog)
            }
            (3, Presence::Present(MapOnly(catalog)), Presence::Present(MapOnly(markers))) => {
                markers.bind(catalog.bind(config)?).map(Self::Markers)
            }
            _ => Err(()),
        }
    }
}
