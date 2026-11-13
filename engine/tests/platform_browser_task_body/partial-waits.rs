//! Test-owned row locks only: no DDL, trigger, product hook, altered timeout or backend kill.
use super::fixture::*;
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::time::Duration;

const OWNER_APP: &str = "cyanrex-body-partial-lock-fixture";
pub(super) const TASK_STATEMENT: &str = "FROM collaboration_tasks WHERE task_id";
pub(super) const SESSION_STATEMENT: &str = "JOIN users u ON u.username = s.username";

// These are dedicated, unpooled connections. Drop closes the socket, so PostgreSQL rolls
// back a held transaction even if an assertion panics before the explicit release.
pub(super) struct Held {
    connection: PgConnection,
    pub(super) pid: i32,
}
pub(super) struct Queued {
    pending: tokio::task::JoinHandle<Held>,
    pub(super) pid: i32,
}
impl Drop for Queued {
    fn drop(&mut self) {
        // Dropping a plain JoinHandle would detach it and leave its SQL lock alive.
        self.pending.abort();
    }
}

async fn connection(f: &Fixture) -> Held {
    let options = f
        .owner()
        .connect_options()
        .as_ref()
        .clone()
        .application_name(OWNER_APP);
    let mut connection = within(PgConnection::connect_with(&options)).await.unwrap();
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    let pid = within(query("SELECT pg_backend_pid() AS pid").fetch_one(&mut connection))
        .await
        .unwrap()
        .get("pid");
    Held { connection, pid }
}

pub(super) async fn task_gate(f: &Fixture) -> Held {
    let mut held = connection(f).await;
    within(
        query(&format!(
            "SELECT task_id FROM {}.collaboration_tasks WHERE task_id=$1 FOR UPDATE",
            f.task
        ))
        .bind(task_id(10).as_uuid())
        .fetch_one(&mut held.connection),
    )
    .await
    .unwrap();
    held
}

pub(super) async fn queue_after_first(
    f: &Fixture,
    first: i32,
    digest: &str,
    revoked: bool,
) -> Queued {
    let mut held = connection(f).await;
    let pid = held.pid;
    let statement = if revoked {
        format!(
            "SELECT token FROM {}.sessions WHERE token=$1 FOR UPDATE",
            f.source
        )
    } else {
        format!(
            "SELECT task_id FROM {}.collaboration_tasks WHERE task_id=$1 FOR UPDATE",
            f.task
        )
    };
    let checked_statement = statement.clone();
    let digest = digest.to_owned();
    let pending = tokio::spawn(async move {
        if revoked {
            within(
                query(&statement)
                    .bind(digest)
                    .fetch_one(&mut held.connection),
            )
            .await
            .unwrap();
        } else {
            within(
                query(&statement)
                    .bind(task_id(10).as_uuid())
                    .fetch_one(&mut held.connection),
            )
            .await
            .unwrap();
        }
        held
    });
    let queued = Queued { pending, pid };
    let username = f.owner().connect_options().get_username().to_owned();
    tokio::time::timeout(Duration::from_millis(900), async {
        loop {
            let hit: bool = within(query("SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity WHERE pid=$1 AND datname=$2 AND usename=$3 AND application_name=$4 AND wait_event_type='Lock' AND $5=ANY(pg_catalog.pg_blocking_pids(pid)) AND pg_catalog.strpos(query,$6)>0) AS hit")
                .bind(pid).bind(&f.database).bind(&username).bind(OWNER_APP).bind(first)
                .bind(&checked_statement).fetch_one(f.owner())).await.unwrap().get("hit");
            if hit { break; }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.expect("the dedicated owner row waiter must queue behind the exact first platform backend");
    queued
}

pub(super) async fn platform_wait(f: &Fixture, blocker: i32, statement: &str) -> i32 {
    tokio::time::timeout(Duration::from_millis(900), async {
        loop {
            let rows = within(query("SELECT pid FROM pg_catalog.pg_stat_activity WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection' AND wait_event_type='Lock' AND $3=ANY(pg_catalog.pg_blocking_pids(pid)) AND pg_catalog.strpos(query,$4)>0")
                .bind(&f.database).bind(&f.role).bind(blocker).bind(statement).fetch_all(f.owner())).await.unwrap();
            if !rows.is_empty() { assert_eq!(rows.len(), 1); return rows[0].get("pid"); }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.expect("the exact second request must wait on the observed owner row fence")
}

impl Queued {
    pub(super) async fn acquire(mut self) -> Held {
        tokio::time::timeout(Duration::from_millis(900), &mut self.pending)
            .await
            .expect("the owner row lock must follow first COMMIT promptly")
            .unwrap()
    }
}
impl Held {
    pub(super) async fn release(mut self) {
        within(query("ROLLBACK").execute(&mut self.connection))
            .await
            .unwrap();
        within(self.connection.close()).await.unwrap();
    }
    pub(super) async fn revoke(mut self, f: &Fixture, digest: &str) {
        let deleted = within(
            query(&format!("DELETE FROM {}.sessions WHERE token=$1", f.source))
                .bind(digest)
                .execute(&mut self.connection),
        )
        .await
        .unwrap();
        assert_eq!(deleted.rows_affected(), 1);
        within(query("COMMIT").execute(&mut self.connection))
            .await
            .unwrap();
        within(self.connection.close()).await.unwrap();
    }
}
