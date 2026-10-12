use super::support::*;
use crate::{fixture::*, *};
use std::time::Duration;
async fn configure_pool(pool: &PgPool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = pool.acquire().await.unwrap();
        query("SET default_transaction_isolation='repeatable read'")
            .execute(&mut *c)
            .await
            .unwrap();
        query("SET synchronous_commit='off'")
            .execute(&mut *c)
            .await
            .unwrap();
        held.push(c);
    }
}
async fn backend_ids(pool: &PgPool) -> Vec<i32> {
    let mut held = Vec::new();
    let mut result = Vec::new();
    for _ in 0..4 {
        let mut c = pool.acquire().await.unwrap();
        result.push(
            query("SELECT pg_backend_pid() AS n")
                .fetch_one(&mut *c)
                .await
                .unwrap()
                .get("n"),
        );
        held.push(c);
    }
    result
}
async fn both_wait(f: &Fixture, pid: i32, a: &[i32], b: &[i32]) -> bool {
    tokio::time::timeout(Duration::from_secs(1),async{loop{let r=query("WITH RECURSIVE waiting AS (SELECT pid FROM pg_stat_activity WHERE application_name=$1 AND state='active' AND wait_event_type='Lock'), chains(writer,pid,visited) AS (SELECT w.pid,b.pid,ARRAY[w.pid,b.pid] FROM waiting w CROSS JOIN LATERAL unnest(pg_blocking_pids(w.pid)) b(pid) UNION ALL SELECT c.writer,b.pid,c.visited||b.pid FROM chains c CROSS JOIN LATERAL unnest(pg_blocking_pids(c.pid)) b(pid) WHERE NOT b.pid=ANY(c.visited)) SELECT count(DISTINCT writer) FILTER(WHERE writer=ANY($3)) AS a,count(DISTINCT writer) FILTER(WHERE writer=ANY($4)) AS b FROM chains WHERE pid=$2").bind(&f.base.grants.auth.base.schema).bind(pid).bind(a).bind(b).fetch_one(&f.base.grants.auth.base.admin).await.unwrap();if r.get::<i64,_>("a")==1&&r.get::<i64,_>("b")==1{return true;}tokio::task::yield_now().await;}}).await.unwrap_or(false)
}
async fn settings_pool(pool: &PgPool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = pool.acquire().await.unwrap();
        let r=query("SELECT current_setting('default_transaction_isolation') AS i,current_setting('synchronous_commit') AS s,current_setting('default_transaction_read_only') AS r").fetch_one(&mut *c).await.unwrap();
        assert_eq!(r.get::<String, _>("i"), "repeatable read");
        assert_eq!(r.get::<String, _>("s"), "off");
        assert_eq!(r.get::<String, _>("r"), "off");
        held.push(c);
    }
}
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_sharing_queued_writers_have_single_quota_and_cas_winners_with_fresh_defaults(
) {
    checked(Fixture::with_max(2).await,|f|Box::pin(async move{
        let old=issue(f,1).await.unwrap();let pool=f.auxiliary_pool(&f.base.grants.auth.base.schema).await;let other=DurableAuthSource::new(pool.clone(),authority());configure(f,false).await;configure_pool(&pool).await;let original_pids=backend_ids(&f.base.grants.auth.base.pool).await;let other_pids=backend_ids(&pool).await;
        let before=f.snapshot().await;let mut holder=f.base.grants.auth.base.admin.begin().await.unwrap();let pid=backend(&mut holder).await;query(&format!("LOCK TABLE {}.{HEADS} IN ACCESS EXCLUSIVE MODE",f.schema)).execute(&mut *holder).await.unwrap();
        let c2=f.command(2);let c3=f.command(3);let mut a=Box::pin(f.base.grants.auth.source.issue_session_file_recipient_grant(&f.base.grants.auth.learner_token,&f.workspace,&c2));let mut b=Box::pin(other.issue_session_file_recipient_grant(&f.base.grants.auth.learner_token,&f.workspace,&c3));let mut early_a=None;let mut early_b=None;
        let waited=tokio::select!{r=&mut a=>{early_a=Some(r);false},r=&mut b=>{early_b=Some(r);false},w=both_wait(f,pid,&original_pids,&other_pids)=>w};holder.commit().await.unwrap();let(a_result,b_result)=tokio::time::timeout(Duration::from_secs(5),async{tokio::join!(async{match early_a{Some(r)=>r,None=>a.as_mut().await}},async{match early_b{Some(r)=>r,None=>b.as_mut().await}})}).await.unwrap();drop(a);drop(b);
        let after=f.snapshot().await;let mut winner=None;for(seed,result)in[(2,a_result),(3,b_result)]{match result{Ok(record)=>{assert!(winner.replace(seed).is_none());assert_eq!(record.grant().id(),recipient_grant(seed));assert_eq!(record.issuer(),f.actor());assert_eq!(record.grant().publisher(),f.actor());assert_eq!(record.grant().recipient(),f.recipient);assert_eq!(record.grant().file(),&f.expected_file);assert_eq!(get(f,seed,f.recipient).await.unwrap(),Some(record));},Err(e)=>{assert_eq!(e,recipient(FileRecipientGrantStoreError::QuotaExceeded));assert_eq!(get(f,seed,f.recipient).await.unwrap(),None);}}}let winner=winner.unwrap();recipient_delta(&f.schema,&before,&after,winner,false);assert!(waited,"both independent source backends must have recursive actual Lock chains to this holder");
        assert_eq!(issue(f,winner).await,Err(recipient(FileRecipientGrantStoreError::Conflict)));assert_eq!(issue(f,4).await,Err(recipient(FileRecipientGrantStoreError::QuotaExceeded)));
        let before=f.snapshot().await;let mut holder=f.base.grants.auth.base.admin.begin().await.unwrap();let pid=backend(&mut holder).await;query(&format!("LOCK TABLE {}.{HEADS} IN ACCESS EXCLUSIVE MODE",f.schema)).execute(&mut *holder).await.unwrap();
        let mut a=Box::pin(f.base.grants.auth.source.revoke_session_file_recipient_grant(&f.base.grants.auth.learner_token,&f.workspace,recipient_grant(1),revision(1)));let mut b=Box::pin(other.revoke_session_file_recipient_grant(&f.base.grants.auth.learner_token,&f.workspace,recipient_grant(1),revision(1)));let mut early_a=None;let mut early_b=None;
        let waited=tokio::select!{r=&mut a=>{early_a=Some(r);false},r=&mut b=>{early_b=Some(r);false},w=both_wait(f,pid,&original_pids,&other_pids)=>w};holder.commit().await.unwrap();let(a_result,b_result)=tokio::time::timeout(Duration::from_secs(5),async{tokio::join!(async{match early_a{Some(r)=>r,None=>a.as_mut().await}},async{match early_b{Some(r)=>r,None=>b.as_mut().await}})}).await.unwrap();drop(a);drop(b);
        let mut winners=0;for result in[a_result,b_result]{match result{Ok(record)=>{winners+=1;assert_eq!(record.grant().state(),FileGrantState::Revoked);assert_eq!(record.issuer(),old.issuer());assert_eq!(record.grant().file(),old.grant().file());assert_eq!(get(f,1,f.recipient).await.unwrap(),Some(record));},Err(e)=>assert_eq!(e,recipient(FileRecipientGrantStoreError::Conflict))}}assert_eq!(winners,1);assert!(waited);recipient_delta(&f.schema,&before,&f.snapshot().await,1,true);assert_eq!(issue(f,4).await,Err(recipient(FileRecipientGrantStoreError::QuotaExceeded)),"revoked records never free retained slots");settings(f,false).await;settings_pool(&pool).await;pool.close().await;
    })).await;
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            issue(f, 1).await.unwrap();
            configure(f, true).await;
            let before = f.snapshot().await;
            let expected = session(SessionCommandError::Authentication(
                DurableAuthError::StorageUnavailable,
            ));
            assert_eq!(issue(f, 2).await, Err(expected));
            assert_eq!(revoke(f, 1, revision(1)).await, Err(expected));
            assert_eq!(f.snapshot().await, before);
            settings(f, true).await;
            configure(f, false).await;
            issue(f, 2).await.unwrap();
            revoke(f, 1, revision(1)).await.unwrap();
            settings(f, false).await;
        })
    })
    .await;
}
