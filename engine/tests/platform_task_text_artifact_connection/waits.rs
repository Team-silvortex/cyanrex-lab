use super::fixture::*;
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::time::Duration;

async fn holder(f: &Fixture, sql: &str) -> (PgConnection, i32) {
    let mut connection = within(PgConnection::connect_with(
        f.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap();
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    within(query(sql).fetch_one(&mut connection)).await.unwrap();
    let pid = within(query("SELECT pg_backend_pid() AS pid").fetch_one(&mut connection))
        .await
        .unwrap()
        .get("pid");
    (connection, pid)
}

async fn blocked(f: &Fixture, blocker: i32, statement: &str) -> i32 {
    tokio::time::timeout(Duration::from_secs(2), async { loop {
        let rows = within(query("SELECT pid FROM pg_catalog.pg_stat_activity WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection' AND wait_event_type='Lock' AND $3=ANY(pg_catalog.pg_blocking_pids(pid)) AND pg_catalog.strpos(query,$4)>0")
            .bind(&f.database).bind(&f.role).bind(blocker).bind(statement).fetch_all(f.owner())).await.unwrap();
        if !rows.is_empty() { assert_eq!(rows.len(), 1); return rows[0].get("pid"); }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }}).await.expect("must observe the exact database/role/app/backend statement and blocker")
}

async fn crossed(f: &Fixture, deadline: chrono::DateTime<chrono::Utc>) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if within(
                query("SELECT clock_timestamp()>=$1 AS crossed")
                    .bind(deadline)
                    .fetch_one(f.owner()),
            )
            .await
            .unwrap()
            .get::<bool, _>("crossed")
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("owned database clock must cross the explicit deadline");
}

async fn release(mut connection: PgConnection) {
    within(query("ROLLBACK").execute(&mut connection))
        .await
        .unwrap();
    within(connection.close()).await.unwrap();
}

async fn session_already_sampled(f: &Fixture, token: &str) {
    let mut connection = within(PgConnection::connect_with(
        f.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap();
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    let locked = within(
        query(&format!(
            "SELECT token FROM {}.sessions WHERE token=$1 FOR UPDATE NOWAIT",
            f.source
        ))
        .bind(token_hash(token))
        .fetch_one(&mut connection),
    )
    .await;
    assert!(
        locked.err().is_some_and(|error| error
            .as_database_error()
            .is_some_and(|database| database.code().as_deref() == Some("55P03"))),
        "the current Session row must already be locked before the Task marker wait"
    );
    release(connection).await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_observed_task_wait_rechecks_session_before_new_file()
{
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await;
        let old = p.task(10, 0, "Wait on an owned Task before publication").await;
        let connector = p.connector().await;
        let (marker, marker_pid) = holder(f, &format!("SELECT singleton FROM {}.collaboration_task_schema WHERE singleton FOR UPDATE", f.task)).await;
        let (task, task_pid) = holder(f, &format!("SELECT task_id FROM {}.collaboration_tasks WHERE task_id='{}' FOR UPDATE", f.task, old.task.reference.task_id)).await;
        let expiry: chrono::DateTime<chrono::Utc> = within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+interval '3 seconds' WHERE token=$1 RETURNING expires_at", f.source))
            .bind(token_hash(&p.tokens[0])).fetch_one(f.owner())).await.unwrap().get("expires_at");
        let before = snapshot(f).await; let files = p.files();
        let original = old.clone(); let token = p.tokens[0].clone();
        let pending = tokio::spawn(async move {
            connector.create_session_task_text_artifact(&token, original, artifact_id(1), revision_id(1),
                "Never materialize after expired Task wait".into()).await
        });
        let backend = blocked(f, marker_pid, "collaboration_task_schema WHERE singleton FOR SHARE").await;
        assert_ne!(backend, marker_pid); assert_ne!(backend, task_pid);
        session_already_sampled(f, &p.tokens[0]).await;
        assert_eq!(files, p.files(), "no new bytes while waiting for the original Task marker");
        // Two observed locks widen initial validity without exceeding the unchanged product
        // lock_timeout=2s at either fence. Both releases follow absolute database-clock times.
        crossed(f, expiry - chrono::Duration::milliseconds(1250)).await;
        release(marker).await;
        assert_eq!(blocked(f, task_pid, "FROM collaboration_tasks WHERE task_id").await, backend);
        assert_eq!(files, p.files(), "no new bytes before Task ownership and fresh Session check");
        crossed(f, expiry).await;
        release(task).await;
        assert_eq!(pending.await.unwrap(), Err(invalid_session()));
        assert_eq!(before, snapshot(f).await); assert_eq!(files, p.files());
        p.unchanged(&old).await; p.counts(0).await;
        assert!(super::storage_helpers::no_connections(f).await); assert!(p.finish());
    })).await;
}
