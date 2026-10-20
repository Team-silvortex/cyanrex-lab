use super::fixture::*;
use chrono::{DateTime, Utc};
use cyanrex_engine::models::collaboration::TaskRef;
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::time::Duration;

pub(super) async fn holder(f: &Fixture, task: TaskRef) -> (PgConnection, i32) {
    hold(
        f,
        &format!(
            "SELECT task_id FROM {}.collaboration_tasks WHERE task_id=$1 FOR UPDATE",
            f.task
        ),
        task.task_id.as_uuid(),
    )
    .await
}
async fn hold(f: &Fixture, sql: &str, id: uuid::Uuid) -> (PgConnection, i32) {
    let mut connection = within(PgConnection::connect_with(
        f.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap();
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    within(query(sql).bind(id).fetch_one(&mut connection))
        .await
        .unwrap();
    let pid = within(query("SELECT pg_backend_pid() AS pid").fetch_one(&mut connection))
        .await
        .unwrap()
        .get("pid");
    (connection, pid)
}
pub(super) async fn blocked(f: &Fixture, blocker: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let waiting = within(
                query(
                    "SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity
                WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection'
                AND $3=ANY(pg_catalog.pg_blocking_pids(pid))) AS waiting",
                )
                .bind(&f.database)
                .bind(&f.role)
                .bind(blocker)
                .fetch_one(f.owner()),
            )
            .await
            .unwrap()
            .get::<bool, _>("waiting");
            if waiting {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or(false)
}
pub(super) async fn release(mut connection: PgConnection) {
    within(query("ROLLBACK").execute(&mut connection))
        .await
        .unwrap();
    within(connection.close()).await.unwrap();
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_content_observed_task_and_artifact_waits_recheck_final_expiry() {
    for mode in ["task", "artifact"] {
        Fixture::checked(|f| Box::pin(async move {
            let p = Prepared::new(f).await;
            let artifact = p.artifact(0, b"Content must not leave after Session expiry").await;
            let task = p.task_manifest(10, 0, manifest("最终授权是读取边界", vec![artifact.reference.clone()])).await;
            let connector = p.connector().await;
            assert!(p.read(&connector, 0, &task).await.unwrap().is_some());
            let (fence, pid) = if mode == "task" { holder(f, task.task.reference).await } else {
                hold(f, &format!("SELECT artifact_id FROM {}.collaboration_artifacts WHERE artifact_id=$1 FOR UPDATE", f.artifacts), artifact.reference.artifact_id.as_uuid()).await
            };
            let expiry: DateTime<Utc> = within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+interval '800 milliseconds' WHERE token=$1 RETURNING expires_at", f.source))
                .bind(token_hash(&p.tokens[0])).fetch_one(f.owner())).await.unwrap().get("expires_at");
            let before = snapshot(f).await;
            let files = p.files();
            let (result, waited) = tokio::join!(
                connector.get_session_content_task(&p.tokens[0], task.task.reference.task_id),
                async {
                    let waited = blocked(f, pid).await;
                    let crossed = tokio::time::timeout(Duration::from_secs(2), async {
                        loop {
                            if within(query("SELECT clock_timestamp()>=$1 AS elapsed").bind(expiry).fetch_one(f.owner())).await.unwrap().get::<bool,_>("elapsed") { return true; }
                            tokio::time::sleep(Duration::from_millis(5)).await;
                        }
                    }).await.unwrap_or(false);
                    release(fence).await;
                    assert!(crossed, "PostgreSQL clock must cross real expiry before fence release");
                    waited
                }
            );
            assert!(waited, "must observe this command blocked by the exact {mode} row fence");
            assert_eq!(result, Err(invalid_session()));
            assert!(before == snapshot(f).await);
            assert!(files == p.files());
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })).await;
    }
}
