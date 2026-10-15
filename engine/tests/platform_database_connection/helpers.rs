use super::catalog_fixture::{within, Fixture};
use cyanrex_engine::platform_host::{
    inspect_platform_database, PlatformCatalogObservation, PlatformDatabaseConnectionError,
    PlatformDatabaseConnector, PlatformDatabaseEndpoint,
};
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;

const PASSWORD: &str = "synthetic-platform-catalog-fixture-only";

pub(super) fn endpoint(fixture: &Fixture) -> PlatformDatabaseEndpoint {
    let options = fixture.runtime().connect_options();
    match options.get_socket() {
        Some(directory) => PlatformDatabaseEndpoint::unix(directory, options.get_port()),
        None => PlatformDatabaseEndpoint::loopback(
            options
                .get_host()
                .parse()
                .expect("explicit fixture transport must be a literal loopback address"),
            options.get_port(),
        ),
    }
    .unwrap_or_else(|_| panic!("explicit fixture endpoint must satisfy local transport policy"))
}

pub(super) async fn connector(
    fixture: &Fixture,
) -> Result<PlatformDatabaseConnector, PlatformDatabaseConnectionError> {
    let original = fixture
        .observe()
        .await
        .unwrap_or_else(|_| panic!("owned original catalog observation unavailable"));
    PlatformDatabaseConnector::new(
        fixture.config().clone(),
        original,
        endpoint(fixture),
        PASSWORD,
    )
}

pub(super) async fn attempt(
    connector: &PlatformDatabaseConnector,
) -> Result<(), PlatformDatabaseConnectionError> {
    match within(connector.connect()).await {
        Ok(connection) => {
            within(connection.close())
                .await
                .unwrap_or_else(|_| panic!("owned connector connection did not close"));
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub(super) async fn identity(
    fixture: &Fixture,
    original: &PlatformCatalogObservation,
    connection: &mut PgConnection,
) -> (i32, bool, bool) {
    let row = within(query("SELECT pg_catalog.pg_backend_pid() AS pid,
        current_user::text = $1 AND session_user::text = $1 AS exact_role,
        current_database()::text = $2 AS exact_database,
        current_setting('search_path') = $3 AS exact_path,
        current_schemas(false) = ARRAY[$3]::name[] AS source_only,
        current_setting('application_name') = 'cyanrex-platform-connection' AS application,
        current_setting('transaction_read_only') = 'on'
            AND current_setting('default_transaction_read_only') = 'on' AS read_only,
        (SELECT setting = '5000' FROM pg_catalog.pg_settings WHERE name = 'statement_timeout') AS statement_bound,
        (SELECT setting = '2000' FROM pg_catalog.pg_settings WHERE name = 'lock_timeout') AS lock_bound,
        (SELECT setting = '10000' FROM pg_catalog.pg_settings WHERE name = 'idle_in_transaction_session_timeout') AS idle_bound,
        NOT has_database_privilege(current_user, current_database(), 'CREATE')
            AND NOT has_database_privilege(current_user, current_database(), 'TEMP') AS no_database_writes,
        pg_my_temp_schema() = 0 AS no_temp")
        .bind(&fixture.role).bind(&fixture.database).bind(&fixture.source)
        .fetch_one(&mut *connection)).await
        .unwrap_or_else(|_| panic!("independent connector identity oracle unavailable"));
    let policy = [
        "exact_role",
        "exact_database",
        "exact_path",
        "source_only",
        "application",
        "read_only",
        "statement_bound",
        "lock_bound",
        "idle_bound",
        "no_database_writes",
        "no_temp",
    ]
    .into_iter()
    .all(|column| row.get::<bool, _>(column));
    let observed = within(inspect_platform_database(fixture.config(), connection)).await;
    (
        row.get("pid"),
        policy,
        observed
            .as_ref()
            .is_ok_and(|current| original.matches(current)),
    )
}

pub(super) fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
