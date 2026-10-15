use super::catalog_fixture::{within, Fixture};
use cyanrex_engine::{
    platform_host::{
        PlatformCatalogExpectation, PlatformDatabaseConnectionError, PlatformDatabaseConnector,
        PlatformDatabaseEndpoint, PlatformDatabaseTarget, PlatformHostPreflightConfig,
        PlatformHostPreflightError, PlatformNamespaces,
    },
    services::auth_service::durable_source::SessionStockLimits,
};
use serde_json::{json, Value};
use sqlx_core::{connection::Connection, executor::Executor, query::query, row::Row};
use std::time::Duration;
use uuid::Uuid;

pub(super) const PASSWORD: &str = "synthetic-platform-catalog-fixture-only";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Pins {
    pub(super) role: u32,
    pub(super) database_owner: u32,
    pub(super) namespaces: [u32; 3],
    pub(super) owners: [u32; 3],
}

impl Pins {
    /// Independent administrator catalog queries, never a runtime-role observer baseline.
    pub(super) async fn read(fixture: &Fixture) -> Self {
        let row = within(
            query(
                "SELECT r.oid::bigint AS role,
            (SELECT d.datdba::bigint FROM pg_catalog.pg_database d WHERE d.datname=$2) AS owner
            FROM pg_catalog.pg_roles r WHERE r.rolname=$1",
            )
            .bind(&fixture.role)
            .bind(&fixture.database)
            .fetch_one(fixture.owner()),
        )
        .await
        .unwrap();
        let rows = within(
            query(
                "SELECT nspname::text AS name, oid::bigint AS oid,
            nspowner::bigint AS owner FROM pg_catalog.pg_namespace WHERE nspname=ANY($1)",
            )
            .bind(vec![
                fixture.source.clone(),
                fixture.task.clone(),
                fixture.artifacts.clone(),
            ])
            .fetch_all(fixture.owner()),
        )
        .await
        .unwrap();
        let names = [&fixture.source, &fixture.task, &fixture.artifacts];
        let mut namespaces = [0; 3];
        let mut owners = [0; 3];
        for row in rows {
            let name: String = row.get("name");
            let index = names
                .iter()
                .position(|expected| **expected == name)
                .unwrap();
            namespaces[index] = u32::try_from(row.get::<i64, _>("oid")).unwrap();
            owners[index] = u32::try_from(row.get::<i64, _>("owner")).unwrap();
        }
        Self {
            role: u32::try_from(row.get::<i64, _>("role")).unwrap(),
            database_owner: u32::try_from(row.get::<i64, _>("owner")).unwrap(),
            namespaces,
            owners,
        }
    }

    pub(super) fn expectation(
        self,
        fixture: &Fixture,
    ) -> Result<PlatformCatalogExpectation, PlatformHostPreflightError> {
        PlatformCatalogExpectation::new(
            fixture.config().clone(),
            self.role,
            self.database_owner,
            self.namespaces,
            self.owners,
        )
    }

    pub(super) fn json(self) -> Value {
        json!({"role_oid": self.role, "database_owner_oid": self.database_owner,
            "source": {"oid": self.namespaces[0], "owner_oid": self.owners[0]},
            "task": {"oid": self.namespaces[1], "owner_oid": self.owners[1]},
            "artifacts": {"oid": self.namespaces[2], "owner_oid": self.owners[2]}})
    }

    pub(super) fn wrong(self, index: usize) -> Self {
        let mut changed = self;
        let wrong = 1_000_000_000 + u32::try_from(index).unwrap();
        match index {
            0 => changed.role = wrong,
            1 => changed.database_owner = wrong,
            2..=4 => changed.namespaces[index - 2] = wrong,
            5..=7 => changed.owners[index - 5] = wrong,
            _ => unreachable!(),
        }
        changed
    }
}

fn endpoint(fixture: &Fixture) -> PlatformDatabaseEndpoint {
    let options = fixture.runtime().connect_options();
    match options.get_socket() {
        Some(path) => PlatformDatabaseEndpoint::unix(path, options.get_port()),
        None => PlatformDatabaseEndpoint::loopback(
            options.get_host().parse().unwrap(),
            options.get_port(),
        ),
    }
    .unwrap_or_else(|_| panic!("explicit fixture local endpoint unavailable"))
}

pub(super) fn connector(
    fixture: &Fixture,
    pins: Pins,
) -> Result<PlatformDatabaseConnector, PlatformDatabaseConnectionError> {
    PlatformDatabaseConnector::with_catalog_expectation(
        fixture.config().clone(),
        pins.expectation(fixture).unwrap(),
        endpoint(fixture),
        PASSWORD,
    )
}

pub(super) fn mismatch(
    fixture: &Fixture,
    pins: Pins,
    config: PlatformHostPreflightConfig,
) -> Result<(), PlatformDatabaseConnectionError> {
    PlatformDatabaseConnector::with_catalog_expectation(
        config,
        pins.expectation(fixture).unwrap(),
        endpoint(fixture),
        PASSWORD,
    )
    .map(|_| ())
}

pub(super) async fn attempt(
    connector: &PlatformDatabaseConnector,
) -> Result<(), PlatformDatabaseConnectionError> {
    match within(connector.connect()).await {
        Ok(connection) => {
            within(connection.close()).await.unwrap();
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub(super) async fn no_connections(fixture: &Fixture) -> bool {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let row = query(
                "SELECT count(*) AS count FROM pg_catalog.pg_stat_activity
                WHERE datname=$1 AND application_name='cyanrex-platform-connection'",
            )
            .bind(&fixture.database)
            .fetch_one(fixture.owner())
            .await
            .unwrap();
            if row.get::<i64, _>("count") == 0 {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or(false)
}

pub(super) fn config_variants(fixture: &Fixture) -> Vec<PlatformHostPreflightConfig> {
    (0..12)
        .map(|index| {
            let old = fixture.config();
            let mut scope = old.scope();
            if index == 1 {
                scope.authority_id = Uuid::from_u128(7301).try_into().unwrap();
            }
            if index == 2 {
                scope.workspace_id = Uuid::from_u128(7302).try_into().unwrap();
            }
            PlatformHostPreflightConfig::new(
                if index == 0 {
                    "https://different.example"
                } else {
                    old.origin()
                },
                scope,
                PlatformNamespaces::new(
                    if index == 3 {
                        "different_source"
                    } else {
                        &fixture.source
                    },
                    if index == 4 {
                        "different_task"
                    } else {
                        &fixture.task
                    },
                    if index == 5 {
                        "different_artifacts"
                    } else {
                        &fixture.artifacts
                    },
                )
                .unwrap(),
                PlatformDatabaseTarget::new(
                    if index == 6 {
                        "different_database"
                    } else {
                        &fixture.database
                    },
                    if index == 7 {
                        "different_role"
                    } else {
                        &fixture.role
                    },
                    old.target().system_identifier() + u64::from(index == 8),
                    old.target().database_oid() + u32::from(index == 9),
                )
                .unwrap(),
                SessionStockLimits::new(
                    old.limits().per_account() + u16::from(index == 10),
                    old.limits().total() + u16::from(index == 11),
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect()
}

pub(super) fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub(super) async fn prepare_replacement_role(fixture: &mut Fixture) -> String {
    let role = fixture.additional_role().await;
    fixture
        .base_sql(&format!("ALTER ROLE {role} LOGIN PASSWORD '{PASSWORD}'"))
        .await;
    fixture
        .sql(&format!(
            "ALTER ROLE {role} IN DATABASE {} SET default_transaction_read_only=on",
            fixture.database
        ))
        .await;
    fixture
        .sql(&format!(
            "GRANT CONNECT ON DATABASE {} TO {role}",
            fixture.database
        ))
        .await;
    fixture
        .sql(&format!(
            "GRANT EXECUTE ON FUNCTION pg_catalog.pg_control_system() TO {role}"
        ))
        .await;
    for namespace in [&fixture.source, &fixture.task, &fixture.artifacts] {
        fixture
            .sql(&format!("GRANT USAGE ON SCHEMA {namespace} TO {role}"))
            .await;
    }
    role
}

pub(super) async fn swap_roles(fixture: &Fixture, auxiliary: &str) -> bool {
    let temporary = format!("catalog_swap_{}", Uuid::new_v4().simple());
    let mut connection = within(fixture.owner().acquire()).await.unwrap();
    // One Simple Query batch has one implicit transaction. Error rolls all changes back,
    // so both registered names remain available for cleanup. RENAME clears MD5 passwords;
    // reset only these two owned synthetic credentials so rejection still tests catalog OIDs.
    within(Executor::execute(&mut *connection, format!(
        "ALTER ROLE {runtime} RENAME TO {temporary}; ALTER ROLE {auxiliary} RENAME TO {runtime}; ALTER ROLE {temporary} RENAME TO {auxiliary}; ALTER ROLE {runtime} PASSWORD '{PASSWORD}'; ALTER ROLE {auxiliary} PASSWORD '{PASSWORD}'",
        runtime = fixture.role).as_str())).await.is_ok()
}
