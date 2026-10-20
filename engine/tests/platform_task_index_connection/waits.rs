use super::fixture::*;
use chrono::{DateTime, Utc};
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::time::Duration;

pub(super) async fn holder(
    f: &Fixture,
    task: cyanrex_engine::models::collaboration::TaskRef,
) -> (PgConnection, i32) {
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
            "SELECT task_id FROM {}.collaboration_tasks WHERE task_id=$1 FOR UPDATE",
            f.task
        ))
        .bind(task.task_id.as_uuid())
        .fetch_one(&mut connection),
    )
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
            let value = within(
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
            if value {
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
async fn postgres_platform_task_index_observed_task_wait_rechecks_expiry_and_original_namespace_owner(
) {
    for mode in ["expiry", "owner"] {
        Fixture::checked(|f| Box::pin(async move {
            let auxiliary = f.additional_role().await;
            let original_owner = super::expectation_helpers::quote_identifier(&f.owner_name().await);
            let p = Prepared::new(f).await;
            let task = p.task(10, 0, "Pending private metadata must stay private").await;
            let connector = p.connector().await;
            assert_eq!(p.page(&connector, 0, None, None).await.unwrap().entries, vec![entry(&task)]);
            let (fence, pid) = holder(f, task.task.reference).await;
            let expiry: Option<DateTime<Utc>> = if mode == "expiry" {
                Some(within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+interval '800 milliseconds' WHERE token=$1 RETURNING expires_at", f.source))
                    .bind(token_hash(&p.tokens[0])).fetch_one(f.owner())).await.unwrap().get("expires_at"))
            } else { None };
            let before = snapshot(f).await;
            let (result, waited) = tokio::join!(
                connector.list_session_content_tasks(&p.tokens[0], None, None),
                async {
                    let waited = blocked(f, pid).await;
                    if let Some(expiry) = expiry {
                        let elapsed = tokio::time::timeout(Duration::from_secs(2), async {
                            loop {
                                if within(query("SELECT clock_timestamp()>=$1 AS elapsed").bind(expiry)
                                    .fetch_one(f.owner())).await.unwrap().get::<bool,_>("elapsed") { return true; }
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                        }).await.unwrap_or(false);
                        assert!(elapsed, "PostgreSQL clock must cross actual expiry before releasing the owned row");
                    } else {
                        f.sql(&format!("ALTER SCHEMA {} OWNER TO {auxiliary}", f.task)).await;
                    }
                    release(fence).await;
                    waited
                }
            );
            assert!(waited, "must observe this runtime role blocked by the exact Task row fence");
            assert_eq!(result, Err(if mode == "expiry" { invalid_session() } else { unavailable() }));
            assert!(before == snapshot(f).await, "failed read changed private business rows");
            if mode == "owner" {
                f.sql(&format!("ALTER SCHEMA {} OWNER TO {original_owner}", f.task)).await;
                assert_eq!(p.page(&connector, 0, None, None).await.unwrap().entries, vec![entry(&task)]);
            }
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })).await;
    }
}
