use super::{
    catalog_fixture::{within, Fixture},
    expectation_helpers::Pins,
};
use cyanrex_engine::platform_host::{
    inspect_platform_database, inspect_platform_storage_markers,
    inspect_platform_storage_markers_expected, PlatformDatabaseEndpoint,
    PlatformHostPreflightError, PlatformStorageMarkerExpectation, PlatformStorageMarkerObservation,
};
use serde_json::{json, Value};
use sqlx_core::{query::query, row::Row};

pub(super) const PASSWORD: &str = "synthetic-platform-catalog-fixture-only";
pub(super) const TABLES: [&str; 5] = [
    "collaboration_auth_source_schema",
    "collaboration_identity_schema",
    "collaboration_access_schema",
    "collaboration_task_schema",
    "collaboration_artifact_schema",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct MarkerPins {
    pub(super) oids: [u32; 5],
    pub(super) owners: [u32; 5],
}
impl MarkerPins {
    pub(super) async fn read(fixture: &Fixture) -> Self {
        let mut oids = [0; 5];
        let mut owners = [0; 5];
        for index in 0..5 {
            let row = within(
                query(
                    "SELECT c.oid::bigint AS oid,c.relowner::bigint AS owner
                FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
                WHERE n.nspname=$1 AND c.relname=$2",
                )
                .bind(namespace(fixture, index))
                .bind(TABLES[index])
                .fetch_one(fixture.owner()),
            )
            .await
            .unwrap();
            oids[index] = u32::try_from(row.get::<i64, _>("oid")).unwrap();
            owners[index] = u32::try_from(row.get::<i64, _>("owner")).unwrap();
        }
        Self { oids, owners }
    }
    pub(super) fn expectation(
        self,
        fixture: &Fixture,
        catalog: Pins,
    ) -> PlatformStorageMarkerExpectation {
        PlatformStorageMarkerExpectation::new(
            catalog.expectation(fixture).unwrap(),
            self.oids,
            self.owners,
        )
        .unwrap()
    }
    pub(super) fn wrong(self, index: usize) -> Self {
        let mut changed = self;
        let wrong = 1_100_000_000 + u32::try_from(index).unwrap();
        if index < 5 {
            changed.oids[index] = wrong;
        } else {
            changed.owners[index - 5] = wrong;
        }
        changed
    }
    pub(super) fn json(self) -> Value {
        let mut value = serde_json::Map::new();
        for (index, name) in ["source", "identity", "access", "task", "artifacts"]
            .into_iter()
            .enumerate()
        {
            value.insert(
                name.into(),
                json!({"oid":self.oids[index],"owner_oid":self.owners[index]}),
            );
        }
        Value::Object(value)
    }
}

pub(super) fn namespace(fixture: &Fixture, index: usize) -> &str {
    match index {
        0..=2 => &fixture.source,
        3 => &fixture.task,
        4 => &fixture.artifacts,
        _ => unreachable!(),
    }
}
pub(super) fn relation(fixture: &Fixture, index: usize) -> String {
    format!("{}.{}", namespace(fixture, index), TABLES[index])
}
pub(super) fn endpoint(fixture: &Fixture) -> PlatformDatabaseEndpoint {
    let options = fixture.runtime().connect_options();
    match options.get_socket() {
        Some(path) => PlatformDatabaseEndpoint::unix(path, options.get_port()),
        None => PlatformDatabaseEndpoint::loopback(
            options.get_host().parse().unwrap(),
            options.get_port(),
        ),
    }
    .unwrap()
}
pub(super) async fn observe(
    fixture: &Fixture,
    expected: &PlatformStorageMarkerExpectation,
) -> Result<PlatformStorageMarkerObservation, PlatformHostPreflightError> {
    let mut connection = within(fixture.runtime().acquire()).await.unwrap();
    within(inspect_platform_storage_markers_expected(
        fixture.config(),
        &mut connection,
        expected,
    ))
    .await
}
pub(super) async fn current(
    fixture: &Fixture,
) -> Result<PlatformStorageMarkerObservation, PlatformHostPreflightError> {
    let mut connection = within(fixture.runtime().acquire()).await.unwrap();
    let catalog = within(inspect_platform_database(fixture.config(), &mut connection)).await?;
    within(inspect_platform_storage_markers(
        fixture.config(),
        &mut connection,
        &catalog,
    ))
    .await
}
pub(super) async fn no_connections(fixture: &Fixture) -> bool {
    super::expectation_helpers::no_connections(fixture).await
}
