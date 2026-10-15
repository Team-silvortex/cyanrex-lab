use super::{fixture::Restricted, http_fixture::*};
use serde_json::json;
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_permissions_source_guard_holds_three_relation_locks_and_blocks_real_ddl()
{
    let (before, after, reply, restored, exact_locks, ddl_waited, ddl_finished, clean_slots, acl) =
        Fixture::checked(|f| Box::pin(async move {
            Restricted::checked(f, |r| Box::pin(async move {
                let before = r.owner.snapshot().await;
                let mut holder = within(r.owner.pool().begin()).await
                    .unwrap_or_else(|_| panic!("source guard holder unavailable"));
                within(query("SELECT singleton FROM collaboration_auth_source_schema FOR UPDATE")
                    .fetch_one(&mut *holder)).await
                    .unwrap_or_else(|_| panic!("source guard holder failed"));
                let blocker: i32 = within(query("SELECT pg_backend_pid() AS pid")
                    .fetch_one(&mut *holder)).await
                    .unwrap_or_else(|_| panic!("source guard PID unavailable")).get("pid");
                let app = r.app();
                let req = request("GET", SESSION, &r.owner.token);
                let pending = Pending::start(async move { capture(app, req).await });
                let runtime_pid = r.blocked_by(blocker).await;
                let exact_locks = within(query("SELECT count(DISTINCT relation)=3 AS exact
                    FROM pg_catalog.pg_locks WHERE pid=$1 AND locktype='relation'
                    AND database=(SELECT oid FROM pg_catalog.pg_database WHERE datname=current_database())
                    AND mode='AccessShareLock' AND granted AND relation IN
                    (pg_catalog.to_regclass($2||'.users'),pg_catalog.to_regclass($2||'.sessions'),
                     pg_catalog.to_regclass($2||'.collaboration_auth_source_schema'))")
                    .bind(runtime_pid).bind(&r.owner.schema).fetch_one(r.owner.admin())).await
                    .unwrap_or_else(|_| panic!("exact runtime relation lock observation failed")).get::<bool,_>("exact");
                let ddl_pool = r.owner.pool().clone();
                let ddl = Pending::start(async move {
                    let mut tx = within(ddl_pool.begin()).await
                        .unwrap_or_else(|_| panic!("owned DDL transaction unavailable"));
                    let locked = within(query("LOCK TABLE users, sessions, collaboration_auth_source_schema IN ACCESS EXCLUSIVE MODE")
                        .execute(&mut *tx)).await.is_ok();
                    within(tx.rollback()).await
                        .unwrap_or_else(|_| panic!("owned DDL transaction rollback failed"));
                    locked
                });
                let ddl_pid = r.owner.blocked_by(runtime_pid).await;
                let ddl_waited = ddl_pid>0 && !ddl.is_finished() && !pending.is_finished();
                within(holder.rollback()).await
                    .unwrap_or_else(|_| panic!("source guard release failed"));
                let reply = pending.finish().await;
                let ddl_finished = ddl.finish().await;
                let restored = capture(r.app(), request("GET", SESSION, &r.owner.token)).await;
                let clean_slots = r.no_owned_prepared_slots().await;
                let acl = r.minimal_acl().await;
                let after = r.owner.snapshot().await;
                (before, after, reply, restored, exact_locks, ddl_waited, ddl_finished, clean_slots, acl)
            })).await
        })).await;
    assert!(
        exact_locks && ddl_waited && ddl_finished,
        "parse-only pin did not retain exact relation locks through the observed Source guard wait"
    );
    successful(
        &reply,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    successful(
        &restored,
        json!({"schema_version":1,"authenticated":true}),
        false,
    );
    assert!(
        clean_slots && acl,
        "successful pin left a prepared slot or broadened privileges"
    );
    assert!(
        unchanged(&before, &after),
        "read/DDL-lock controls changed Source state"
    );
}
