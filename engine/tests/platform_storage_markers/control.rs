use super::fixture::{within, Fixture};
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::{future::Future, time::Duration};

pub(super) struct Pending<T> {
    job: Option<tokio::task::JoinHandle<T>>,
}

impl<T: Send + 'static> Pending<T> {
    pub(super) fn start(future: impl Future<Output = T> + Send + 'static) -> Self {
        Self {
            job: Some(tokio::spawn(future)),
        }
    }
    pub(super) async fn finish(mut self) -> T {
        let result = within(self.job.as_mut().unwrap()).await;
        self.job.take();
        result.unwrap_or_else(|_| panic!("owned marker inspection task failed"))
    }
}

impl<T> Drop for Pending<T> {
    fn drop(&mut self) {
        if let Some(job) = &self.job {
            job.abort();
        }
    }
}

pub(super) struct Admin {
    connection: Option<PgConnection>,
    pub(super) pid: i32,
}

impl Admin {
    pub(super) async fn open(fixture: &Fixture) -> Self {
        let options = fixture.owner().connect_options();
        let mut connection = within(PgConnection::connect_with(options.as_ref()))
            .await
            .unwrap_or_else(|_| panic!("owned independent SQL fence connection unavailable"));
        let pid = within(query("SELECT pg_backend_pid() AS pid").fetch_one(&mut connection))
            .await
            .unwrap_or_else(|_| panic!("owned SQL fence identity unavailable"))
            .get("pid");
        Self {
            connection: Some(connection),
            pid,
        }
    }
    pub(super) fn connection(&mut self) -> &mut PgConnection {
        self.connection.as_mut().unwrap()
    }
    pub(super) async fn close(mut self, fixture: &Fixture) {
        let closed = within(self.connection.take().unwrap().close())
            .await
            .is_ok();
        within(async {
            loop {
                let gone: bool = query("SELECT NOT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity WHERE pid=$1) AS gone")
                    .bind(self.pid).fetch_one(fixture.owner()).await
                    .unwrap_or_else(|_| panic!("owned SQL fence shutdown observation failed")).get("gone");
                if gone { break; }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await;
        assert!(closed, "owned SQL fence must close before database cleanup");
    }
}

pub(super) async fn wait_for_relation_lock(
    fixture: &Fixture,
    waiter: i32,
    blocker: i32,
    relation: i64,
) -> bool {
    within(async {
        loop {
            let blocked: bool = query("SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity a
                JOIN pg_catalog.pg_locks l ON l.pid=a.pid
                WHERE a.pid=$1 AND a.datname=current_database() AND a.wait_event_type='Lock'
                AND $2=ANY(pg_catalog.pg_blocking_pids(a.pid)) AND l.locktype='relation'
                AND l.relation::bigint=$3 AND l.mode='AccessShareLock' AND NOT l.granted) AS blocked")
                .bind(waiter).bind(blocker).bind(relation).fetch_one(fixture.owner()).await
                .unwrap_or_else(|_| panic!("actual marker ACCESS SHARE waiter observation failed")).get("blocked");
            if blocked { return true; }
            // The predicate is an actual independently observed server lock, not elapsed time.
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await
}
