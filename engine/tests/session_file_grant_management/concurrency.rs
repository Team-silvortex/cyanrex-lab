use super::{
    fixture::*,
    publisher_fixture::{grant, validity, CHANGES, HEADS},
    support::*,
    *,
};
use std::time::Duration;
async fn configure(f: &Fixture, readonly: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.base.auth.base.pool.acquire().await.unwrap();
        for s in [
            "SET default_transaction_isolation='repeatable read'",
            "SET synchronous_commit='off'",
            if readonly {
                "SET default_transaction_read_only='on'"
            } else {
                "SET default_transaction_read_only='off'"
            },
        ] {
            query(s).execute(&mut *c).await.unwrap();
        }
        held.push(c);
    }
}
async fn settings(f: &Fixture, readonly: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.base.auth.base.pool.acquire().await.unwrap();
        let r=query("SELECT current_setting('default_transaction_isolation') AS i,current_setting('synchronous_commit') AS s,current_setting('default_transaction_read_only') AS r").fetch_one(&mut *c).await.unwrap();
        assert_eq!(r.get::<String, _>("i"), "repeatable read");
        assert_eq!(r.get::<String, _>("s"), "off");
        assert_eq!(r.get::<String, _>("r"), if readonly { "on" } else { "off" });
        held.push(c);
    }
}
async fn chain(f: &Fixture, holder: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(1),async{loop{
        let n:i64=query("WITH RECURSIVE waiting AS (SELECT pid FROM pg_stat_activity WHERE application_name=$1 AND state='active' AND wait_event_type='Lock'),
            blockers(writer,pid,visited) AS (SELECT w.pid,b.pid,ARRAY[w.pid,b.pid] FROM waiting w CROSS JOIN LATERAL unnest(pg_blocking_pids(w.pid)) b(pid)
            UNION ALL SELECT c.writer,b.pid,c.visited || b.pid FROM blockers c CROSS JOIN LATERAL unnest(pg_blocking_pids(c.pid)) b(pid) WHERE NOT b.pid=ANY(c.visited))
            SELECT count(DISTINCT writer) AS n FROM blockers WHERE pid=$2")
            .bind(&f.base.auth.base.schema).bind(holder).fetch_one(&f.base.auth.base.admin).await.unwrap().get("n");
        if n==2{return true;}tokio::task::yield_now().await;
    }}).await.unwrap_or(false)
}
async fn held_heads(f: &Fixture) -> Tx<'_> {
    let mut tx = f.base.auth.base.admin.begin().await.unwrap();
    query(&format!(
        "LOCK TABLE {}.{HEADS} IN ACCESS EXCLUSIVE MODE",
        f.base.schema
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx
}
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_grant_queued_rr_quota_and_revoke_cas_use_fresh_transactions_without_overriding_readonly(
) {
    let f = Fixture::with_max(2).await;
    let old = issue(&f, 1, f.target()).await.unwrap();
    configure(&f, false).await;
    let before = f.base.snapshot().await;
    let mut holder = held_heads(&f).await;
    let pid = backend(&mut holder).await;
    let mut a = Box::pin(f.base.auth.source.issue_session_file_publisher_grant(
        &f.base.auth.learner_token,
        &f.management,
        grant(2),
        f.target(),
        validity(),
    ));
    let mut b = Box::pin(f.base.auth.source.issue_session_file_publisher_grant(
        &f.base.auth.learner_token,
        &f.management,
        grant(3),
        f.manager(),
        validity(),
    ));
    let mut early_a = None;
    let mut early_b = None;
    let waited = tokio::select! {r=&mut a=>{early_a=Some(r);false},r=&mut b=>{early_b=Some(r);false},w=chain(&f,pid)=>w};
    holder.rollback().await.unwrap();
    let (a_result, b_result) = tokio::join!(
        async {
            match early_a {
                Some(r) => r,
                None => a.as_mut().await,
            }
        },
        async {
            match early_b {
                Some(r) => r,
                None => b.as_mut().await,
            }
        }
    );
    drop(a);
    drop(b);
    let after = f.base.snapshot().await;
    let mut winners = 0;
    for (seed, subject, result) in [(2, f.target(), a_result), (3, f.manager(), b_result)] {
        match result {
            Ok(record) => {
                winners += 1;
                assert_eq!(record.grant().id(), grant(seed));
                assert_eq!(record.grant().subject(), subject);
                assert_eq!(record.issuer(), f.manager());
                assert_eq!(record.grant().revision(), revision(1));
                assert_eq!(direct_get(&f, seed, subject).await.unwrap(), Some(record));
            }
            Err(e) => {
                assert_eq!(e, grant_error(FilePublisherGrantStoreError::QuotaExceeded));
                assert!(direct_get(&f, seed, subject).await.unwrap().is_none());
            }
        }
    }
    assert_eq!(winners, 1);
    delta(
        &f,
        &before,
        &after,
        if rows(&after, &f.base.schema, HEADS).iter().any(|r| {
            serde_json::from_str::<serde_json::Value>(r).unwrap()["grant_id"]
                == serde_json::json!(grant(2))
        }) {
            2
        } else {
            3
        },
        false,
    );
    assert_eq!(direct_get(&f, 1, f.target()).await.unwrap(), Some(old));
    assert_eq!(
        issue(&f, 4, f.target()).await,
        Err(grant_error(FilePublisherGrantStoreError::QuotaExceeded))
    );
    assert_eq!(
        issue(&f, 1, f.target()).await,
        Err(grant_error(FilePublisherGrantStoreError::Conflict))
    );
    settings(&f, false).await;
    let before = f.base.snapshot().await;
    let mut holder = held_heads(&f).await;
    let pid = backend(&mut holder).await;
    let mut a = Box::pin(f.base.auth.source.revoke_session_file_publisher_grant(
        &f.base.auth.learner_token,
        &f.management,
        grant(1),
        revision(1),
    ));
    let mut b = Box::pin(f.base.auth.source.revoke_session_file_publisher_grant(
        &f.base.auth.learner_token,
        &f.management,
        grant(1),
        revision(1),
    ));
    let mut early_a = None;
    let mut early_b = None;
    let cas_waited = tokio::select! {r=&mut a=>{early_a=Some(r);false},r=&mut b=>{early_b=Some(r);false},w=chain(&f,pid)=>w};
    holder.rollback().await.unwrap();
    let (ra, rb) = tokio::join!(
        async {
            match early_a {
                Some(r) => r,
                None => a.as_mut().await,
            }
        },
        async {
            match early_b {
                Some(r) => r,
                None => b.as_mut().await,
            }
        }
    );
    drop(a);
    drop(b);
    assert_eq!([&ra, &rb].iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        [ra, rb].into_iter().find_map(Result::err),
        Some(grant_error(FilePublisherGrantStoreError::Conflict))
    );
    delta(&f, &before, &f.base.snapshot().await, 1, true);
    assert_eq!(
        issue(&f, 4, f.target()).await,
        Err(grant_error(FilePublisherGrantStoreError::QuotaExceeded)),
        "revocation retains its global slot"
    );
    settings(&f, false).await;
    restored(&f).await;
    f.cleanup().await;
    assert!(waited && cas_waited);

    let f = Fixture::ready().await;
    issue(&f, 1, f.target()).await.unwrap();
    configure(&f, true).await;
    let before = f.base.snapshot().await;
    let write = issue(&f, 2, f.target()).await;
    let revoke_result = revoke(&f, 1, revision(1)).await;
    assert!(write.is_err() && revoke_result.is_err());
    assert_eq!(f.base.snapshot().await, before);
    settings(&f, true).await;
    configure(&f, false).await;
    issue(&f, 2, f.target()).await.unwrap();
    revoke(&f, 1, revision(1)).await.unwrap();
    settings(&f, false).await;
    // This probe observes sync=on at the real deferred-flush point, not power-loss durability
    // or an arbitrary acknowledgement-loss boundary. Defaults must remain untouched afterward.
    fault(&f,CHANGES,"INSERT","IF current_setting('synchronous_commit') <> 'on' THEN RAISE EXCEPTION 'sync probe'; END IF; RETURN NEW;",true).await;
    issue(&f, 3, f.target()).await.unwrap();
    assert!(hit(&f).await);
    remove_fault(&f, CHANGES).await;
    settings(&f, false).await;
    f.cleanup().await;
}
