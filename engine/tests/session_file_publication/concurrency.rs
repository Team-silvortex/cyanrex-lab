use super::{fixture::*, support::*, *};
use cyanrex_engine::services::file_upload_publication::FileUploadPublicationStoreError;
use std::time::Duration;

async fn configure(pool: &PgPool, statement: &str) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = pool.acquire().await.unwrap();
        query(statement).execute(&mut *c).await.unwrap();
        held.push(c);
    }
}
async fn settings(pool: &PgPool, isolation: &str, readonly: &str, sync: &str) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = pool.acquire().await.unwrap();
        let r=query("SELECT current_setting('default_transaction_isolation') AS i,current_setting('default_transaction_read_only') AS r,current_setting('synchronous_commit') AS s").fetch_one(&mut *c).await.unwrap();
        assert_eq!(r.get::<String, _>("i"), isolation);
        assert_eq!(r.get::<String, _>("r"), readonly);
        assert_eq!(r.get::<String, _>("s"), sync);
        held.push(c);
    }
}
async fn two_chains(f: &Fixture, holder: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(2),async{loop{
        let found:i64=query("WITH RECURSIVE waiting AS (SELECT pid FROM pg_stat_activity WHERE application_name=$1 AND state='active' AND wait_event_type='Lock'), chains(writer,pid,visited) AS (SELECT w.pid,b.pid,ARRAY[w.pid,b.pid] FROM waiting w CROSS JOIN LATERAL unnest(pg_blocking_pids(w.pid)) b(pid) UNION ALL SELECT c.writer,b.pid,c.visited||b.pid FROM chains c CROSS JOIN LATERAL unnest(pg_blocking_pids(c.pid)) b(pid) WHERE NOT b.pid=ANY(c.visited)) SELECT count(DISTINCT writer) AS n FROM chains WHERE pid=$2")
            .bind(&f.grants.auth.base.schema).bind(holder).fetch_one(&f.grants.auth.base.admin).await.unwrap().get("n");
        if found==2{return true;}tokio::task::yield_now().await;
    }}).await.unwrap_or(false)
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_publication_queued_rr_writers_use_fresh_retained_quota_and_preserve_readonly_defaults(
) {
    let f = Fixture::with_limits(0, 4, 2).await;
    f.grants.issue(1, f.actor()).await;
    healthy(&f).await;
    let ma = f.stage_additional(2, 0, f.actor()).await;
    let mb = f.stage_additional(3, 0, f.actor()).await;
    let mut a = prepare(&f, 2).await;
    let mut b = prepare(&f, 3).await;
    configure(
        &f.grants.auth.base.pool,
        "SET default_transaction_isolation='repeatable read'",
    )
    .await;
    configure(&f.grants.auth.base.pool, "SET synchronous_commit='off'").await;
    let before = f.snapshot().await;
    let mut holder = f.grants.auth.base.admin.begin().await.unwrap();
    let pid = backend(&mut holder).await;
    query(&format!(
        "LOCK TABLE {}.{PUBLICATIONS} IN ACCESS EXCLUSIVE MODE",
        f.publication_schema
    ))
    .execute(&mut *holder)
    .await
    .unwrap();
    let mut pa = Box::pin(publish(&f, &mut a));
    let mut pb = Box::pin(publish(&f, &mut b));
    let waited = tokio::select! {_=&mut pa=>false,_=&mut pb=>false,w=two_chains(&f,pid)=>w};
    holder.rollback().await.unwrap();
    let (ra, rb) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(pa.as_mut(), pb.as_mut())
    })
    .await
    .unwrap();
    drop(pa);
    drop(pb);
    let after = f.snapshot().await;
    let mut winner = None;
    for (seed, m, r, state) in [(2, ma, ra, a.state()), (3, mb, rb, b.state())] {
        match r {
            Ok(r) => {
                assert!(winner.replace(seed).is_none());
                assert_eq!(state, FileUploadPublicationState::Confirmed);
                receipt(&r, seed, f.actor(), &m);
                let row = f
                    .publication
                    .get(upload(seed), f.actor().principal(), f.actor().account_id())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(row.manifest(), &m);
                assert_eq!(row.upload_id(), upload(seed));
            }
            Err(e) => {
                assert_eq!(
                    e,
                    publisher_error(FileUploadPublicationError::Publication(
                        FileUploadPublicationStoreError::CapacityExhausted
                    ))
                );
                assert_eq!(state, FileUploadPublicationState::Unknown);
                assert!(f
                    .publication
                    .get(upload(seed), f.actor().principal(), f.actor().account_id())
                    .await
                    .unwrap()
                    .is_none());
            }
        }
    }
    let winning = winner.unwrap();
    delta(&f, &before, &after, winning);
    settings(&f.grants.auth.base.pool, "repeatable read", "off", "off").await;
    f.cleanup().await;
    assert!(
        waited,
        "both actual source writers' recursive lock chains must reach the retained-ledger holder"
    );

    let f = Fixture::ready(0).await;
    f.grants.issue(1, f.actor()).await;
    let mut a = prepare(&f, 1).await;
    configure(
        &f.grants.auth.base.pool,
        "SET default_transaction_read_only='on'",
    )
    .await;
    let before = f.snapshot().await;
    let result = publish(&f, &mut a).await;
    let state = a.state();
    let after = f.snapshot().await;
    settings(&f.grants.auth.base.pool, "read committed", "on", "on").await;
    configure(
        &f.grants.auth.base.pool,
        "SET default_transaction_read_only='off'",
    )
    .await;
    let healthy_result = healthy(&f).await;
    receipt(&healthy_result, 1, f.actor(), &f.manifest);
    f.cleanup().await;
    assert_eq!(
        result.err(),
        Some(session_error(SessionCommandError::Authentication(
            DurableAuthError::StorageUnavailable
        )))
    );
    assert_eq!(state, FileUploadPublicationState::Unknown);
    assert_eq!(
        after, before,
        "caller READ ONLY is not overridden to acquire Session/grant write fences"
    );
}
