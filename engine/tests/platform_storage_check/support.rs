use super::catalog_fixture::{within, Fixture};
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::time::Duration;

pub(super) async fn connection_count(fixture: &Fixture) -> i64 {
    within(
        query(
            "SELECT count(*) AS count FROM pg_catalog.pg_stat_activity
        WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection'",
        )
        .bind(&fixture.database)
        .bind(&fixture.role)
        .fetch_one(fixture.owner()),
    )
    .await
    .unwrap_or_else(|_| panic!("independent owned backend oracle unavailable"))
    .get("count")
}

pub(super) async fn no_connections(fixture: &Fixture) -> bool {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if connection_count(fixture).await == 0 {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or(false)
}

pub(super) async fn marker_state(fixture: &Fixture) -> String {
    within(
        query(
            "SELECT md5(concat_ws('|', c.oid::text, c.relowner::text,
        coalesce(c.relacl::text,''), c.relrowsecurity::text, c.relforcerowsecurity::text,
        (SELECT string_agg(a.attname::text || ':' || a.atttypid::text || ':' || a.attnotnull::text,
            '|' ORDER BY a.attnum) FROM pg_catalog.pg_attribute a
            WHERE a.attrelid=c.oid AND a.attnum>0 AND NOT a.attisdropped))) AS digest
        FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname=$1 AND c.relname='collaboration_auth_source_schema'",
        )
        .bind(&fixture.source)
        .fetch_one(fixture.owner()),
    )
    .await
    .unwrap_or_else(|_| panic!("independent marker metadata snapshot unavailable"))
    .get("digest")
}

pub(super) async fn lock_marker(fixture: &Fixture) -> (PgConnection, i64, i32) {
    let oid = within(
        query(
            "SELECT c.oid::bigint AS oid FROM pg_catalog.pg_class c
        JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
        WHERE n.nspname=$1 AND c.relname='collaboration_auth_source_schema'",
        )
        .bind(&fixture.source)
        .fetch_one(fixture.owner()),
    )
    .await
    .unwrap()
    .get("oid");
    let mut connection = within(PgConnection::connect_with(
        fixture.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap_or_else(|_| panic!("owned independent lock-holder connection unavailable"));
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    within(
        query(&format!(
            "LOCK TABLE {}.collaboration_auth_source_schema IN ACCESS EXCLUSIVE MODE",
            fixture.source
        ))
        .execute(&mut connection),
    )
    .await
    .unwrap();
    let pid = within(query("SELECT pg_catalog.pg_backend_pid() AS pid").fetch_one(&mut connection))
        .await
        .unwrap()
        .get("pid");
    (connection, oid, pid)
}

pub(super) async fn pending_marker_lock(fixture: &Fixture, oid: i64, blocker: i32) -> bool {
    // Release the holder promptly: the production transaction's lock_timeout is 2s.
    tokio::time::timeout(Duration::from_millis(900), async {
        loop {
            let row = query(
                "SELECT EXISTS (SELECT 1 FROM pg_catalog.pg_locks l
                JOIN pg_catalog.pg_stat_activity a ON a.pid=l.pid
                WHERE a.datname=$1 AND a.usename=$2
                AND a.application_name='cyanrex-platform-connection'
                AND l.database=$3::oid AND l.relation=$4::oid
                AND l.locktype='relation' AND l.mode='AccessShareLock' AND NOT l.granted
                AND $5=ANY(pg_catalog.pg_blocking_pids(a.pid))) AS pending",
            )
            .bind(&fixture.database)
            .bind(&fixture.role)
            .bind(i64::from(fixture.config().target().database_oid()))
            .bind(oid)
            .bind(blocker)
            .fetch_one(fixture.owner())
            .await;
            if row.is_ok_and(|row| row.get::<bool, _>("pending")) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or(false)
}

pub(super) async fn release(mut connection: PgConnection) -> bool {
    let released = within(query("ROLLBACK").execute(&mut connection))
        .await
        .is_ok();
    let closed = within(connection.close()).await.is_ok();
    released && closed
}
