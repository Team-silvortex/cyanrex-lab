use super::{
    catalog_fixture::{within, Fixture},
    fixture::{otp, password, session_count, snapshot, username, watermark, Prepared},
    storage_helpers::no_connections,
};
use cyanrex_engine::services::auth_service::durable_source::DurableAuthError;
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::time::Duration;

async fn holder(f: &Fixture) -> (PgConnection, i32) {
    let mut connection = within(PgConnection::connect_with(
        f.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap();
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    within(
        query(&format!(
            "SELECT singleton FROM {}.collaboration_auth_source_schema FOR UPDATE",
            f.source
        ))
        .fetch_one(&mut connection),
    )
    .await
    .unwrap();
    let pid = within(query("SELECT pg_catalog.pg_backend_pid() AS pid").fetch_one(&mut connection))
        .await
        .unwrap()
        .get("pid");
    (connection, pid)
}

async fn pending(f: &Fixture, pid: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let row = query(
                "SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity
                WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection'
                AND $3=ANY(pg_catalog.pg_blocking_pids(pid))) AS pending",
            )
            .bind(&f.database)
            .bind(&f.role)
            .bind(pid)
            .fetch_one(f.owner())
            .await
            .unwrap();
            if row.get::<bool, _>("pending") {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or(false)
}

async fn release(mut connection: PgConnection) -> bool {
    let rolled_back = within(query("ROLLBACK").execute(&mut connection))
        .await
        .is_ok();
    let closed = within(connection.close()).await.is_ok();
    rolled_back && closed
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_rechecks_original_catalog_after_observed_source_wait() {
    let (failed, waited, released, unchanged, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            let auxiliary = f.additional_role().await;
            let original = super::expectation_helpers::quote_identifier(&f.owner_name().await);
            let prepared = Prepared::new(f).await;
            let connector = prepared.connector().await;
            let before = snapshot(f).await;
            let (holder, pid) = holder(f).await;
            let name = username();
            let code = otp();
            let (result, (waited, changed, released)) =
                tokio::join!(connector.login(&name, password(), &code), async {
                    let waited = pending(f, pid).await;
                    f.sql(&format!("ALTER SCHEMA {} OWNER TO {auxiliary}", f.source))
                        .await;
                    let changed = snapshot(f).await;
                    let released = release(holder).await;
                    (waited, changed, released)
                });
            let failed = result.map(|_| ());
            let mut unchanged = changed == snapshot(f).await
                && watermark(f).await == -1
                && session_count(f).await == 0;
            f.sql(&format!("ALTER SCHEMA {} OWNER TO {original}", f.source))
                .await;
            unchanged &= before == snapshot(f).await;
            drop(connector);
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (failed, waited, released, unchanged, no_backend, cleaned)
        })
    })
    .await;
    assert_eq!(failed, Err(DurableAuthError::StorageUnavailable));
    assert!(waited && released && unchanged && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_login_connection_cancelled_source_wait_closes_connection_without_otp_write(
) {
    let (waited, cancelled, released, unchanged, no_backend, cleaned) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let connector = prepared.connector().await;
            let before = snapshot(f).await;
            let (holder, pid) = holder(f).await;
            let job = tokio::spawn(async move {
                connector
                    .login(&username(), password(), &otp())
                    .await
                    .map(|_| ())
            });
            let waited = pending(f, pid).await;
            job.abort();
            let cancelled = matches!(job.await,Err(error) if error.is_cancelled());
            let released = release(holder).await;
            let no_backend = no_connections(f).await;
            let unchanged = before == snapshot(f).await
                && watermark(f).await == -1
                && session_count(f).await == 0;
            let cleaned = prepared.finish();
            (waited, cancelled, released, unchanged, no_backend, cleaned)
        })
    })
    .await;
    assert!(waited && cancelled && released && unchanged && no_backend && cleaned);
}
