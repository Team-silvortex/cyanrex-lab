//! Version-gated strict declarations; absence is distinct from an explicitly supplied null.
use super::MapOnly;
use cyanrex_engine::platform_host::{PlatformCatalogExpectation, PlatformHostPreflightConfig};
use serde::{Deserialize, Deserializer};

#[derive(Default)]
pub(super) enum ExpectedField {
    #[default]
    Absent,
    Present(MapOnly<Identity>),
}

impl<'de> Deserialize<'de> for ExpectedField {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        MapOnly::<Identity>::deserialize(deserializer).map(Self::Present)
    }
}

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

impl ExpectedField {
    pub(super) fn bind(
        self,
        version: u8,
        config: &PlatformHostPreflightConfig,
    ) -> Result<Option<PlatformCatalogExpectation>, ()> {
        match (version, self) {
            (1, Self::Absent) => Ok(None),
            (2, Self::Present(MapOnly(identity))) => PlatformCatalogExpectation::new(
                config.clone(),
                identity.role_oid,
                identity.database_owner_oid,
                [
                    identity.source.0.oid,
                    identity.task.0.oid,
                    identity.artifacts.0.oid,
                ],
                [
                    identity.source.0.owner_oid,
                    identity.task.0.owner_oid,
                    identity.artifacts.0.owner_oid,
                ],
            )
            .map(Some)
            .map_err(|_| ()),
            _ => Err(()),
        }
    }
}
