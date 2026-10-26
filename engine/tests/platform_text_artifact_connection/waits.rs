use super::fixture::*;
use sqlx_core::{connection::Connection, query::query, row::Row};
use sqlx_postgres::PgConnection;
use std::{fs, os::unix::fs::PermissionsExt, sync::Arc, time::Duration};

pub(super) async fn holder(f: &Fixture) -> (PgConnection, i32) {
    f.sql(&format!("CREATE FUNCTION {0}.text_connection_pause() RETURNS trigger LANGUAGE plpgsql SET search_path=pg_catalog AS $$ BEGIN PERFORM pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext('{0}'),931); RETURN NEW; END $$",f.artifacts)).await;
    // Deliberately deferred: the original writer's first readback has already completed.
    f.sql(&format!("CREATE CONSTRAINT TRIGGER text_connection_pause AFTER INSERT ON {0}.collaboration_artifact_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION {0}.text_connection_pause()",f.artifacts)).await;
    let mut connection = within(PgConnection::connect_with(
        f.owner().connect_options().as_ref(),
    ))
    .await
    .unwrap();
    within(query("BEGIN").execute(&mut connection))
        .await
        .unwrap();
    within(
        query("SELECT pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtext($1),931)")
            .bind(&f.artifacts)
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
pub(super) async fn blocked(f: &Fixture, blocker: i32) -> i32 {
    tokio::time::timeout(Duration::from_secs(3),async {
        loop {
            if let Some(row)=within(query("SELECT pid FROM pg_catalog.pg_stat_activity WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection' AND $3=ANY(pg_catalog.pg_blocking_pids(pid))")
                .bind(&f.database).bind(&f.role).bind(blocker).fetch_optional(f.owner())).await.unwrap() {return row.get("pid");}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.expect("must observe the actual selected SQL lock wait")
}
pub(super) async fn release(mut connection: PgConnection) {
    within(query("ROLLBACK").execute(&mut connection))
        .await
        .unwrap();
    within(connection.close()).await.unwrap();
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_observed_deferred_waits_recheck_expiry_bytes_root_and_cancel(
) {
    for mode in ["expiry", "bytes", "root", "cancel"] {
        Fixture::checked(|f|Box::pin(async move {
            let p=Prepared::new(f).await; let connector=Arc::new(p.connector().await);
            let witness=p.create(&connector,0,1,"Committed witness").await.unwrap(); let original=p.files();
            let (fence,pid)=holder(f).await;
            let expiry=if mode=="expiry" {
                Some(within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+interval '800 milliseconds' WHERE token=$1 RETURNING expires_at",f.source))
                    .bind(token_hash(&p.tokens[0])).fetch_one(f.owner())).await.unwrap().get::<chrono::DateTime<chrono::Utc>,_>("expires_at"))
            } else {None};
            let before=snapshot(f).await; let text="Unconfirmed exact text"; let reference=p.reference(2,text.as_bytes());
            let c=Arc::clone(&connector); let token=p.tokens[0].clone();
            let pending=tokio::spawn(async move {c.create_session_text_artifact(&token,artifact_id(2),revision_id(2),"Deferred wait".into(),text.into()).await});
            blocked(f,pid).await; assert!(!pending.is_finished()); p.old_files_unchanged(&original,1);
            assert_eq!(fs::read(p.blob(&reference)).unwrap(),text.as_bytes());
            if let Some(expiry)=expiry {
                tokio::time::timeout(Duration::from_secs(2),async { loop {
                    if within(query("SELECT clock_timestamp()>=$1 AS expired").bind(expiry).fetch_one(f.owner())).await.unwrap().get::<bool,_>("expired") {break;}
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }}).await.unwrap();
            }
            if mode=="bytes" {
                let file=p.blob(&reference); fs::set_permissions(&file,fs::Permissions::from_mode(0o600)).unwrap();
                fs::write(&file,vec![b'X';text.len()]).unwrap(); fs::set_permissions(&file,fs::Permissions::from_mode(0o400)).unwrap();
            }
            if mode=="root" {assert!(p.root.swap());}
            if mode=="cancel" {
                pending.abort(); assert!(pending.await.unwrap_err().is_cancelled()); release(fence).await;
            } else {
                release(fence).await; let result=pending.await.unwrap();
                if mode=="expiry" {assert_eq!(result,Err(invalid_session()));} else {assert!(result.is_err(),"{mode}");}
            }
            assert!(super::storage_helpers::no_connections(f).await);
            if mode=="root" {assert!(p.root.restore());}
            assert_eq!(before,snapshot(f).await); p.counts(1).await; p.old_files_unchanged(&original,1);
            assert_eq!(fs::read(p.blob(&reference)).unwrap(),if mode=="bytes" {vec![b'X';text.len()]}else{text.as_bytes().to_vec()});
            p.read_committed(&witness,b"Committed witness").await;
            drop(connector); assert!(p.finish());
        })).await;
    }
}
