//! Strict marker record and pure construction from a complete catalog assertion.
use super::MapOnly;
use cyanrex_engine::platform_host::{PlatformCatalogExpectation, PlatformStorageMarkerExpectation};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Identity {
    source: MapOnly<Marker>,
    identity: MapOnly<Marker>,
    access: MapOnly<Marker>,
    task: MapOnly<Marker>,
    artifacts: MapOnly<Marker>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    oid: u32,
    owner_oid: u32,
}

impl Identity {
    pub(super) fn bind(
        self,
        catalog: PlatformCatalogExpectation,
    ) -> Result<PlatformStorageMarkerExpectation, ()> {
        // Fixed order is the existing Source, Identity, Access, Task, Artifact contract.
        PlatformStorageMarkerExpectation::new(
            catalog,
            [
                self.source.0.oid,
                self.identity.0.oid,
                self.access.0.oid,
                self.task.0.oid,
                self.artifacts.0.oid,
            ],
            [
                self.source.0.owner_oid,
                self.identity.0.owner_oid,
                self.access.0.owner_oid,
                self.task.0.owner_oid,
                self.artifacts.0.owner_oid,
            ],
        )
        .map_err(|_| ())
    }
}
