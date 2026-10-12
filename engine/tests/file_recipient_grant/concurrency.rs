use super::cleanup::checked;
use super::{fixture::*, support::*, *};
use std::time::Duration;
async fn backends(pool: &PgPool) -> Vec<i32> {
    let mut held = Vec::new();
    let mut pids = Vec::new();
    for _ in 0..4 {
        let mut c = pool.acquire().await.unwrap();
        pids.push(
            query("SELECT pg_backend_pid() AS n")
                .fetch_one(&mut *c)
                .await
                .unwrap()
                .get("n"),
        );
        held.push(c);
    }
    pids
}
async fn chains(f: &Fixture, holder: i32, a: &[i32], b: &[i32]) -> bool {
    tokio::time::timeout(Duration::from_secs(1),async{loop{let r=query("WITH RECURSIVE waiting AS (SELECT pid FROM pg_stat_activity WHERE application_name=$1 AND state='active' AND wait_event_type='Lock' AND query LIKE '%LOCK TABLE collaboration_file_recipient_grant_schema%'), chains(writer,pid,visited) AS (SELECT w.pid,b.pid,ARRAY[w.pid,b.pid] FROM waiting w CROSS JOIN LATERAL unnest(pg_blocking_pids(w.pid)) b(pid) UNION ALL SELECT c.writer,b.pid,c.visited || b.pid FROM chains c CROSS JOIN LATERAL unnest(pg_blocking_pids(c.pid)) b(pid) WHERE NOT b.pid=ANY(c.visited)) SELECT count(DISTINCT writer) FILTER(WHERE writer=ANY($3)) AS a,count(DISTINCT writer) FILTER(WHERE writer=ANY($4)) AS b FROM chains WHERE pid=$2")
    .bind(&f.schema).bind(holder).bind(a).bind(b).fetch_one(&f.admin).await.unwrap();if r.get::<i64,_>("a")==1 && r.get::<i64,_>("b")==1{return true;}tokio::task::yield_now().await;}}).await.unwrap_or(false)
}
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_queued_rr_recipient_writers_use_fresh_retained_quota_and_preserve_readonly() {
    checked(ready(2).await,|f|Box::pin(async move{let old=issue(&f,1,account(1),account(2),account(3),validity()).await.unwrap();let(other,pool)=f.reopen().await;
    for p in [&f.pool,&pool]{configure(p,"SET default_transaction_isolation='repeatable read'").await;configure(p,"SET synchronous_commit='off'").await;}
    let a_pids=backends(&f.pool).await;let b_pids=backends(&pool).await;let before=f.snapshot().await;let mut holder=f.admin.begin().await.unwrap();let pid=backend(&mut holder).await;
    query(&format!("LOCK TABLE {}.{HEADS} IN ACCESS EXCLUSIVE MODE",f.schema)).execute(&mut *holder).await.unwrap();
    let mut a=Box::pin(f.store.issue(grant(2),account(2),file(2),account(2),account(2),validity()));let mut b=Box::pin(other.issue(grant(3),account(3),file(3),account(3),account(3),validity()));
    let mut early_a=None;let mut early_b=None;let waited=tokio::select!{r=&mut a=>{early_a=Some(r);false},r=&mut b=>{early_b=Some(r);false},w=chains(&f,pid,&a_pids,&b_pids)=>w};holder.commit().await.unwrap();let(a_result,b_result)=tokio::time::timeout(Duration::from_secs(5),async{tokio::join!(async{match early_a{Some(r)=>r,None=>a.as_mut().await}},async{match early_b{Some(r)=>r,None=>b.as_mut().await}})}).await.unwrap();drop(a);drop(b);let after=f.snapshot().await;
    let mut winner=None;for(seed,result)in[(2,a_result),(3,b_result)]{match result{Ok(record)=>{assert!(winner.replace(seed).is_none());assert_eq!(record.grant().id(),grant(seed));assert_eq!(record.issuer(),account(seed));assert_eq!(record.grant().publisher(),account(seed));assert_eq!(record.grant().recipient(),account(seed));assert_eq!(record.grant().file(),&file(seed));assert_eq!(get(&f,seed,account(seed)).await.unwrap(),Some(record));},Err(e)=>{assert_eq!(e,FileRecipientGrantStoreError::QuotaExceeded);assert_eq!(get(&f,seed,account(seed)).await.unwrap(),None);}}}
    assert!(winner.is_some());assert_eq!(after.oids,before.oids);assert_eq!(rows(&after,MARKER),rows(&before,MARKER));for table in[HEADS,CHANGES]{assert_eq!(rows(&after,table).len(),2);assert!(rows(&after,table).contains(&rows(&before,table)[0]));let delta:serde_json::Value=serde_json::from_str(rows(&after,table).iter().find(|r|!rows(&before,table).contains(r)).unwrap()).unwrap();assert_eq!(delta["grant_id"],serde_json::json!(grant(winner.unwrap())));}
    assert_eq!(get(&f,1,account(3)).await.unwrap(),Some(old));for p in [&f.pool,&pool]{settings(p,"repeatable read","off","off").await;}pool.close().await;assert!(waited,"both independent writers must really wait through recursive blocker chains to the holder");})).await;
    checked(ready(2).await, |f| {
        Box::pin(async move {
            configure(&f.pool, "SET default_transaction_read_only='on'").await;
            let before = f.snapshot().await;
            let failed = f
                .store
                .issue(
                    grant(1),
                    account(1),
                    file(1),
                    account(2),
                    account(3),
                    validity(),
                )
                .await;
            assert_eq!(f.snapshot().await, before);
            assert_eq!(
                failed,
                Err(FileRecipientGrantStoreError::StorageUnavailable)
            );
            assert_eq!(get(&f, 1, account(3)).await.unwrap(), None);
            settings(&f.pool, "read committed", "on", "on").await;
            configure(&f.pool, "SET default_transaction_read_only='off'").await;
            issue(&f, 1, account(1), account(2), account(3), validity())
                .await
                .unwrap();
        })
    })
    .await;
}
