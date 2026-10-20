use super::fixture::*;
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::time::Duration;

pub(super) async fn holder(f: &Fixture) -> (PgConnection, i32) {
    f.sql(&format!("CREATE FUNCTION {0}.platform_create_pause() RETURNS trigger LANGUAGE plpgsql SET search_path=pg_catalog AS $$ BEGIN PERFORM pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext('{0}'),827); RETURN NEW; END $$",f.task)).await;
    f.sql(&format!("CREATE TRIGGER platform_create_pause BEFORE INSERT ON {0}.collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION {0}.platform_create_pause()",f.task)).await;
    let mut connection = within(PgConnection::connect_with(
        f.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap();
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    within(
        query("SELECT pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext($1),827)")
            .bind(&f.task)
            .execute(&mut connection),
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
    tokio::time::timeout(Duration::from_secs(2),async {
        loop {
            if within(query("SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection' AND $3=ANY(pg_catalog.pg_blocking_pids(pid))) AS waiting")
                .bind(&f.database).bind(&f.role).bind(blocker).fetch_one(f.owner())).await.unwrap().get::<bool,_>("waiting") {return true;}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.unwrap_or(false)
}
pub(super) async fn release(mut connection: PgConnection) {
    within(query("ROLLBACK").execute(&mut connection))
        .await
        .unwrap();
    within(connection.close()).await.unwrap();
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_create_observed_event_wait_expiry_and_cancellation_never_confirm() {
    for cancelled in [false, true] {
        Fixture::checked(|f|Box::pin(async move {
            let p=Prepared::new(f).await;
            let connector=p.connector().await;
            let (fence,pid)=holder(f).await;
            let expiry=within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+interval '800 milliseconds' WHERE token=$1 RETURNING expires_at",f.source))
                .bind(token_hash(&p.tokens[0])).fetch_one(f.owner())).await.unwrap().get::<chrono::DateTime<chrono::Utc>,_>("expires_at");
            let before=snapshot(f).await;
            let token=p.tokens[0].clone();
            let pending=tokio::spawn(async move {connector.create_session_empty_content_task(&token,task_id(10),"Must retain final authority").await});
            assert!(blocked(f,pid).await,"must observe actual event-trigger lock wait");
            if cancelled {
                pending.abort();
                assert!(pending.await.unwrap_err().is_cancelled());
                release(fence).await;
            } else {
                tokio::time::timeout(Duration::from_secs(2),async {loop {
                    if within(query("SELECT clock_timestamp()>=$1 AS crossed").bind(expiry).fetch_one(f.owner())).await.unwrap().get::<bool,_>("crossed") {break;}
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }}).await.unwrap();
                assert!(!pending.is_finished());
                release(fence).await;
                assert_eq!(pending.await.unwrap(),Err(invalid_session()));
            }
            assert!(super::storage_helpers::no_connections(f).await);
            assert_eq!(before,snapshot(f).await);
            assert_eq!(p.count("collaboration_tasks").await,0);
            assert_eq!(p.count("collaboration_task_outbox").await,0);
            assert!(p.finish());
        })).await;
    }
}
