//! Strict catalog record and pure construction; version/presence selection has one owner.
use super::MapOnly;
use cyanrex_engine::platform_host::{PlatformCatalogExpectation, PlatformHostPreflightConfig};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Identity {
    role_oid: u32,
    database_owner_oid: u32,
    source: MapOnly<Namespace>,
    task: MapOnly<Namespace>,
    artifacts: MapOnly<Namespace>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Namespace {
    oid: u32,
    owner_oid: u32,
}

impl Identity {
    pub(super) fn bind(
        self,
        config: &PlatformHostPreflightConfig,
    ) -> Result<PlatformCatalogExpectation, ()> {
        PlatformCatalogExpectation::new(
            config.clone(),
            self.role_oid,
            self.database_owner_oid,
            [self.source.0.oid, self.task.0.oid, self.artifacts.0.oid],
            [
                self.source.0.owner_oid,
                self.task.0.owner_oid,
                self.artifacts.0.owner_oid,
            ],
        )
        .map_err(|_| ())
    }
}
