use super::fixture::{snapshot, within, Fixture};
use cyanrex_engine::platform_host::{
    inspect_platform_storage_markers, PlatformCatalogObservation,
    PlatformHostPreflightError as Error,
};
use sqlx_core::{query::query, row::Row};

#[derive(Clone)]
pub(super) struct Marker {
    pub(super) schema: String,
    pub(super) table: &'static str,
    pub(super) version: i32,
    pub(super) scope_columns: usize,
}

impl Marker {
    pub(super) fn name(&self) -> String {
        format!("{}.{}", self.schema, self.table)
    }
    pub(super) fn columns(&self) -> &'static str {
        match self.scope_columns {
            0 => "singleton,version",
            1 => "singleton,version,authority_id",
            _ => "singleton,version,authority_id,workspace_id",
        }
    }
}

pub(super) fn markers(fixture: &Fixture) -> [Marker; 5] {
    [
        Marker {
            schema: fixture.source.clone(),
            table: "collaboration_auth_source_schema",
            version: 2,
            scope_columns: 1,
        },
        Marker {
            schema: fixture.source.clone(),
            table: "collaboration_identity_schema",
            version: 2,
            scope_columns: 0,
        },
        Marker {
            schema: fixture.source.clone(),
            table: "collaboration_access_schema",
            version: 2,
            scope_columns: 0,
        },
        Marker {
            schema: fixture.task.clone(),
            table: "collaboration_task_schema",
            version: 3,
            scope_columns: 2,
        },
        Marker {
            schema: fixture.artifacts.clone(),
            table: "collaboration_artifact_schema",
            version: 1,
            scope_columns: 2,
        },
    ]
}

pub(super) async fn catalog(fixture: &Fixture) -> PlatformCatalogObservation {
    fixture
        .observe()
        .await
        .unwrap_or_else(|_| panic!("original catalog observation unavailable"))
}

pub(super) async fn checked_inspect(
    fixture: &Fixture,
    catalog: &PlatformCatalogObservation,
) -> (Result<(), Error>, bool) {
    let before = snapshot(fixture).await;
    let mut connection = within(fixture.runtime().acquire())
        .await
        .unwrap_or_else(|_| panic!("owned marker inspection connection unavailable"));
    let result = within(inspect_platform_storage_markers(
        fixture.config(),
        &mut connection,
        catalog,
    ))
    .await
    .map(|_| ());
    drop(connection);
    (result, before == snapshot(fixture).await)
}

pub(super) async fn grant_marker(fixture: &Fixture, marker: &Marker) {
    fixture
        .sql(&format!(
            "GRANT SELECT ON {} TO {}",
            marker.name(),
            fixture.role
        ))
        .await;
}

pub(super) async fn restore_row(fixture: &Fixture, marker: &Marker) {
    fixture.sql(&format!("DELETE FROM {}", marker.name())).await;
    let scope = fixture.config().scope();
    let statement = match marker.scope_columns {
        0 => format!(
            "INSERT INTO {} (singleton,version) VALUES (TRUE,$1)",
            marker.name()
        ),
        1 => format!(
            "INSERT INTO {} (singleton,version,authority_id) VALUES (TRUE,$1,$2)",
            marker.name()
        ),
        _ => format!(
            "INSERT INTO {} (singleton,version,authority_id,workspace_id) VALUES (TRUE,$1,$2,$3)",
            marker.name()
        ),
    };
    let mut statement = query(&statement).bind(marker.version);
    if marker.scope_columns > 0 {
        statement = statement.bind(scope.authority_id.as_uuid());
    }
    if marker.scope_columns > 1 {
        statement = statement.bind(scope.workspace_id.as_uuid());
    }
    within(statement.execute(fixture.owner()))
        .await
        .unwrap_or_else(|_| panic!("owned marker row restoration failed"));
}

pub(super) async fn relation_oid(fixture: &Fixture, marker: &Marker) -> i64 {
    within(
        query("SELECT pg_catalog.to_regclass($1)::oid::bigint AS oid")
            .bind(marker.name())
            .fetch_one(fixture.owner()),
    )
    .await
    .unwrap_or_else(|_| panic!("independent marker relation identity unavailable"))
    .get("oid")
}
