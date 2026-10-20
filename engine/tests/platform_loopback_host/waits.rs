use super::{fixture::*, http, wait_helpers};
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_loopback_host_root_swap_during_observed_startup_marker_wait_never_becomes_ready(
) {
    let (pending, swapped, refused, restored, recovered, unchanged, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f).await;
                let before = snapshot(f).await;
                let (holder, oid, blocker) = wait_helpers::lock_marker(f).await;
                let child = prepared.spawn(None);
                let pending = wait_helpers::pending_marker_lock(f, oid, blocker).await;
                let swapped = prepared.root().swap();
                let released = wait_helpers::release(holder).await;
                let run = child.finish(None, Duration::from_secs(15)).await;
                let refused =
                    released && failed(&run) && super::fixture::no_listener(prepared.bind).await;
                let restored = prepared.root().restore();
                // A new explicit startup may select the restored original root; the failed
                // startup must never publish a retained connector from the replaced one.
                let mut next = prepared.spawn(None);
                let ready = next.ready().await;
                let next_run = next.stop().await;
                let recovered = ready && stopped(&next_run);
                let unchanged = before == snapshot(f).await;
                let no_backend = no_connections(f).await;
                let cleaned = prepared.finish();
                (
                    pending, swapped, refused, restored, recovered, unchanged, no_backend, cleaned,
                )
            })
        })
        .await;
    assert!(
        pending
            && swapped
            && refused
            && restored
            && recovered
            && unchanged
            && no_backend
            && cleaned
    );
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_loopback_host_killed_observed_startup_wait_leaves_no_backend_or_mutation(
) {
    let (pending, killed, released, no_port, unchanged, no_backend, cleaned) =
        Fixture::checked(|f| {
            Box::pin(async move {
                let prepared = Prepared::new(f).await;
                let before = snapshot(f).await;
                let (holder, oid, blocker) = wait_helpers::lock_marker(f).await;
                let child = prepared.spawn(None);
                let pending = wait_helpers::pending_marker_lock(f, oid, blocker).await;
                let run = child
                    .finish(Some(libc::SIGKILL), Duration::from_secs(5))
                    .await;
                let killed = killed_before_ready(&run);
                // Release the independent blocker before checking eventual backend cleanup.
                // Socket cancellation alone is not asserted to prove SQL rollback.
                let released = wait_helpers::release(holder).await;
                let no_port = super::fixture::no_listener(prepared.bind).await;
                let no_backend = no_connections(f).await;
                let unchanged = before == snapshot(f).await;
                let cleaned = prepared.finish();
                (
                    pending, killed, released, no_port, unchanged, no_backend, cleaned,
                )
            })
        })
        .await;
    assert!(pending && killed && released && no_port && unchanged && no_backend && cleaned);
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL and non-root capless child support"]
async fn postgres_platform_loopback_host_sigterm_drains_observed_session_work_without_compensation()
{
    let (
        ready,
        issued,
        pending,
        signalled,
        alive,
        released,
        confirmed,
        committed,
        closed,
        no_port,
        no_backend,
        cleaned,
    ) = Fixture::checked(|f| {
        Box::pin(async move {
            let prepared = Prepared::new(f).await;
            let mut child = prepared.spawn(None);
            let ready = child.ready().await;
            let mut issued = false;
            let mut pending = false;
            let mut signalled = false;
            let mut alive = false;
            let mut released = false;
            let mut confirmed = false;
            let mut committed = false;
            if ready {
                let client = http::client();
                let token = http::token(&http::login(&client, &prepared.origin).await);
                issued = token.is_some();
                let token = token.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let counter = watermark(f).await;
                let (holder, blocker) = source_holder(f).await;
                let request = http::logout(&client, &prepared.origin, &token);
                let control = async {
                    let pending = waiting_source(f, blocker).await;
                    let signalled = child.signal(libc::SIGTERM);
                    tokio::time::sleep(Duration::from_millis(30)).await;
                    let alive = !child.poll();
                    let released = wait_helpers::release(holder).await;
                    (pending, signalled, alive, released)
                };
                let (reply, control) = tokio::join!(request, control);
                (pending, signalled, alive, released) = control;
                confirmed = http::cleared(&reply);
                committed = session_count(f).await == 0 && watermark(f).await == counter;
            }
            let closed = stopped(&child.finish(None, Duration::from_secs(35)).await);
            let no_port = super::fixture::no_listener(prepared.bind).await;
            let no_backend = no_connections(f).await;
            let cleaned = prepared.finish();
            (
                ready, issued, pending, signalled, alive, released, confirmed, committed, closed,
                no_port, no_backend, cleaned,
            )
        })
    })
    .await;
    assert!(
        ready
            && issued
            && pending
            && signalled
            && alive
            && released
            && confirmed
            && committed
            && closed
            && no_port
            && no_backend
            && cleaned
    );
}

async fn source_holder(f: &Fixture) -> (PgConnection, i32) {
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

async fn waiting_source(f: &Fixture, blocker: i32) -> bool {
    tokio::time::timeout(Duration::from_millis(900),async {
        loop {
            let pending = query("SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection' AND $3=ANY(pg_catalog.pg_blocking_pids(pid))) AS pending")
                .bind(&f.database).bind(&f.role).bind(blocker).fetch_one(f.owner()).await.unwrap().get::<bool,_>("pending");
            if pending { return true; }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap_or(false)
}
