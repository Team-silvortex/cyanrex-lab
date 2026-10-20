use super::fixture::*;
use cyanrex_engine::models::collaboration::*;
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

pub(super) async fn holder(
    f: &Fixture,
    kind: &str,
    task: TaskId,
    artifact: ArtifactId,
) -> (PgConnection, i32) {
    let mut c = within(PgConnection::connect_with(
        f.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap();
    within(query("BEGIN").execute(&mut c)).await.unwrap();
    if kind == "event" {
        f.sql(&format!("CREATE FUNCTION {0}.platform_replace_pause() RETURNS trigger LANGUAGE plpgsql SET search_path=pg_catalog AS $$ BEGIN PERFORM pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext('{0}'),829); RETURN NEW; END $$",f.task)).await;
        f.sql(&format!("CREATE TRIGGER platform_replace_pause BEFORE INSERT ON {0}.collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION {0}.platform_replace_pause()",f.task)).await;
        within(
            query("SELECT pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext($1),829)")
                .bind(&f.task)
                .execute(&mut c),
        )
        .await
        .unwrap();
    } else {
        let (sql, id) = if kind == "task" {
            (
                format!(
                    "SELECT task_id FROM {}.collaboration_tasks WHERE task_id=$1 FOR UPDATE",
                    f.task
                ),
                task.as_uuid(),
            )
        } else {
            (format!("SELECT artifact_id FROM {}.collaboration_artifacts WHERE artifact_id=$1 FOR UPDATE",f.artifacts),artifact.as_uuid())
        };
        within(query(&sql).bind(id).fetch_one(&mut c))
            .await
            .unwrap();
    }
    let pid = within(query("SELECT pg_backend_pid() AS pid").fetch_one(&mut c))
        .await
        .unwrap()
        .get("pid");
    (c, pid)
}
pub(super) async fn blocked(f: &Fixture, blocker: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(2),async{loop{
        if within(query("SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection' AND $3=ANY(pg_catalog.pg_blocking_pids(pid))) AS waiting")
            .bind(&f.database).bind(&f.role).bind(blocker).fetch_one(f.owner())).await.unwrap().get::<bool,_>("waiting"){return true;}
        tokio::time::sleep(Duration::from_millis(5)).await;
    }}).await.unwrap_or(false)
}
pub(super) async fn release(mut c: PgConnection) {
    within(query("ROLLBACK").execute(&mut c)).await.unwrap();
    within(c.close()).await.unwrap();
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_replace_observed_waits_expiry_cancellation_and_old_new_bytes_fail_closed(
) {
    for kind in [
        "task",
        "artifact",
        "event",
        "cancel",
        "old-bytes",
        "new-bytes",
    ] {
        Fixture::checked(|f|Box::pin(async move {
            let p=Prepared::new(f).await;let a=p.artifact(0,b"Original exact payload").await;let b=p.artifact(0,b"Replacement exact payload").await;
            let old=p.task_manifest(10,0,manifest("Original",vec![a.reference.clone()])).await;
            let connector=p.connector().await;
            let lock_kind=if matches!(kind,"cancel"|"old-bytes"|"new-bytes"){"event"}else{kind};
            let(fence,pid)=holder(f,lock_kind,old.task.reference.task_id,a.reference.artifact_id).await;
            let expiry=if matches!(kind,"task"|"artifact"|"event"){
                Some(within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+interval '800 milliseconds' WHERE token=$1 RETURNING expires_at",f.source))
                    .bind(token_hash(&p.tokens[0])).fetch_one(f.owner())).await.unwrap().get::<chrono::DateTime<chrono::Utc>,_>("expires_at"))
            }else{None};
            let before=snapshot(f).await;let mut files=p.files();let token=p.tokens[0].clone();let original=old.clone();let new_reference=b.reference.clone();
            let pending=tokio::spawn(async move {connector.replace_session_content_task(&token,original.task.reference.task_id,original.task.revision,manifest("Requested edit",vec![b.reference.clone()])).await});
            assert!(blocked(f,pid).await,"must observe exact {kind} blocker");
            if kind=="cancel"{pending.abort();assert!(pending.await.unwrap_err().is_cancelled());release(fence).await;}
            else{
                if let Some(expiry)=expiry{tokio::time::timeout(Duration::from_secs(2),async{loop{
                    if within(query("SELECT clock_timestamp()>=$1 AS crossed").bind(expiry).fetch_one(f.owner())).await.unwrap().get::<bool,_>("crossed"){break;}
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }}).await.unwrap();}
                if matches!(kind,"old-bytes"|"new-bytes"){
                    let name=if kind=="old-bytes"{p.blob(&a.reference)}else{p.blob(&new_reference)};
                    fs::set_permissions(&name,fs::Permissions::from_mode(0o600)).unwrap();fs::write(&name,b"Changed outside the transaction").unwrap();fs::set_permissions(&name,fs::Permissions::from_mode(0o400)).unwrap();files=p.files();
                }
                release(fence).await;let result=pending.await.unwrap();
                if expiry.is_some(){assert_eq!(result,Err(invalid_session()));}else{assert!(result.is_err(),"{kind} must recheck bytes after writes");}
            }
            assert!(super::storage_helpers::no_connections(f).await);assert_eq!(before,snapshot(f).await);assert_eq!(files,p.files());p.unchanged(&old,1).await;assert!(p.finish());
        })).await;
    }
}
