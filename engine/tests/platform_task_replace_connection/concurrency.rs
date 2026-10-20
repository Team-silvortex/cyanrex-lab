use super::fixture::*;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::SessionTaskError,
        task_store::{TaskContentSnapshot, TaskStoreError},
    },
};
use sqlx_core::{query::query, row::Row};
use std::time::Duration;

async fn spawn(
    p: &Prepared<'_>,
    old: &TaskContentSnapshot,
    value: TaskContentManifest,
) -> tokio::task::JoinHandle<Result<TaskContentSnapshot, SessionTaskError>> {
    let c = p.connector().await;
    let token = p.tokens[0].clone();
    let task = old.task.clone();
    tokio::spawn(async move {
        c.replace_session_content_task(&token, task.reference.task_id, task.revision, value)
            .await
    })
}

async fn two_transitive_waiters(f: &Fixture, blocker: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            // PostgreSQL may queue the second row waiter behind the first transaction. Follow
            // the exact blocker graph rather than requiring both requests to name the fence.
            let count: i64 = within(
                query(
                    "WITH RECURSIVE chain(pid) AS (
                SELECT $3::int4 UNION
                SELECT a.pid FROM pg_catalog.pg_stat_activity a JOIN chain c
                    ON c.pid=ANY(pg_catalog.pg_blocking_pids(a.pid)) WHERE a.datname=$1)
                SELECT count(DISTINCT a.pid) AS waiting FROM chain c
                JOIN pg_catalog.pg_stat_activity a ON a.pid=c.pid
                WHERE a.datname=$1 AND a.usename=$2
                    AND a.application_name='cyanrex-platform-connection'",
                )
                .bind(&f.database)
                .bind(&f.role)
                .bind(blocker)
                .fetch_one(f.owner()),
            )
            .await
            .unwrap()
            .get("waiting");
            if count == 2 {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or(false)
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_replace_concurrent_expected_revision_and_opposite_unions_have_no_partial_winners(
) {
    for mode in ["same-task", "opposite-union"] {
        Fixture::checked(|f|Box::pin(async move{
            let p=Prepared::new(f).await;let a=p.artifact(0,b"Original A").await;let b=p.artifact(0,b"Original B").await;
            let first=p.task_manifest(10,0,manifest("First",vec![a.reference.clone()])).await;
            let files=p.files();let immutable=p.immutable().await;
            if mode=="same-task" {
                let(fence,pid)=super::waits::holder(f,"event",first.task.reference.task_id,a.reference.artifact_id).await;
                let one=spawn(&p,&first,manifest("First admitted edit",vec![b.reference.clone()])).await;
                assert!(super::waits::blocked(f,pid).await);
                let first_pid:i32=within(query("SELECT pid FROM pg_catalog.pg_stat_activity WHERE datname=$1 AND usename=$2 AND application_name='cyanrex-platform-connection' AND $3=ANY(pg_catalog.pg_blocking_pids(pid))")
                    .bind(&f.database).bind(&f.role).bind(pid).fetch_one(f.owner())).await.unwrap().get("pid");
                let two=spawn(&p,&first,manifest("Second stale edit",vec![a.reference.clone()])).await;
                assert!(super::waits::blocked(f,first_pid).await,"second command must wait on the exact first command's Task lock");
                super::waits::release(fence).await;
                let winner=one.await.unwrap().unwrap();assert_eq!(winner.manifest.title(),"First admitted edit");
                assert_eq!(two.await.unwrap(),Err(SessionTaskError::Task(TaskStoreError::StaleRevision)));p.unchanged(&winner,2).await;p.reopened(&winner).await;
            }else{
                let second=p.task_manifest(20,0,manifest("Second",vec![b.reference.clone()])).await;
                let minimum=if a.reference.artifact_id.as_uuid()<b.reference.artifact_id.as_uuid(){a.reference.artifact_id}else{b.reference.artifact_id};
                let(fence,pid)=super::waits::holder(f,"artifact",first.task.reference.task_id,minimum).await;
                let one=spawn(&p,&first,manifest("A takes B",vec![b.reference.clone()])).await;
                let two=spawn(&p,&second,manifest("B takes A",vec![a.reference.clone()])).await;
                assert!(two_transitive_waiters(f,pid).await,"both opposite-union requests must overlap at the exact minimum-Artifact fence");
                assert!(!one.is_finished()&&!two.is_finished());
                super::waits::release(fence).await;
                let(one,two)=tokio::time::timeout(Duration::from_secs(5),async{tokio::join!(one,two)}).await.unwrap();
                let one=one.unwrap().unwrap();let two=two.unwrap().unwrap();
                assert_eq!(u64::from(one.task.revision),2);assert_eq!(u64::from(two.task.revision),2);
                p.unchanged(&one,4).await;p.unchanged(&two,4).await;p.reopened(&one).await;p.reopened(&two).await;
            }
            assert_eq!(files,p.files());assert_eq!(immutable,p.immutable().await);
            assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
        })).await;
    }
}
