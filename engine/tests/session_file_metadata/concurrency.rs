use super::{fixture::*, support::*, *};
use std::time::Duration;

async fn configure(f: &Fixture, readonly: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.auth.base.pool.acquire().await.unwrap();
        for statement in [
            "SET default_transaction_isolation='repeatable read'",
            "SET synchronous_commit='off'",
            if readonly {
                "SET default_transaction_read_only='on'"
            } else {
                "SET default_transaction_read_only='off'"
            },
        ] {
            query(statement).execute(&mut *c).await.unwrap();
        }
        held.push(c);
    }
}
async fn settings(f: &Fixture, readonly: bool) {
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut c = f.auth.base.pool.acquire().await.unwrap();
        let row=query("SELECT current_setting('default_transaction_isolation') AS i,current_setting('synchronous_commit') AS s,current_setting('default_transaction_read_only') AS r")
            .fetch_one(&mut *c).await.unwrap();
        assert_eq!(row.get::<String, _>("i"), "repeatable read");
        assert_eq!(row.get::<String, _>("s"), "off");
        assert_eq!(
            row.get::<String, _>("r"),
            if readonly { "on" } else { "off" }
        );
        held.push(c);
    }
}
async fn chain(f: &Fixture, blocker: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(1),async{loop{
        // The scope policy guard serializes authority writers. One caller really waits on the
        // target holder; a second independent source backend really waits behind that caller.
        let count:i64=query("SELECT count(*) AS n FROM pg_stat_activity a JOIN pg_stat_activity b
            ON a.pid=ANY(pg_blocking_pids(b.pid)) WHERE a.application_name=$1 AND b.application_name=$1
            AND $2=ANY(pg_blocking_pids(a.pid)) AND a.pid<>b.pid")
            .bind(&f.auth.base.schema).bind(blocker).fetch_one(&f.auth.base.admin).await.unwrap().get("n");
        if count>0{return true;}tokio::task::yield_now().await;
    }}).await.unwrap_or(false)
}
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_upload_concurrent_quota_reservations_use_fresh_source_transactions() {
    let f = Fixture::with_limits(FileUploadLimits::new(17, 17, 3, 2).unwrap()).await;
    let old = reserve(&f, &f.auth.manager_token, 1, &manifest(0, 1))
        .await
        .unwrap();
    configure(&f, false).await;
    let before = snapshot(&f).await;
    let mut holder = f.auth.base.admin.begin().await.unwrap();
    query(&format!(
        "LOCK TABLE {}.{UPLOAD_ROWS} IN ACCESS EXCLUSIVE MODE",
        f.upload_schema
    ))
    .execute(&mut *holder)
    .await
    .unwrap();
    let pid = backend(&mut holder).await;
    let m2 = manifest(17, 2);
    let m3 = manifest(17, 3);
    let mut a = Box::pin(f.auth.source.reserve_session_file_upload(
        &f.auth.learner_token,
        &f.upload_workspace,
        upload(2),
        &m2,
    ));
    let mut b = Box::pin(f.auth.source.reserve_session_file_upload(
        &f.auth.manager_token,
        &f.upload_workspace,
        upload(3),
        &m3,
    ));
    let mut early_a = None;
    let mut early_b = None;
    let queued = tokio::select! {r=&mut a=>{early_a=Some(r);false},r=&mut b=>{early_b=Some(r);false},value=chain(&f,pid)=>value};
    holder.rollback().await.unwrap();
    // Keep both callers polled: either may hold the shared authority lock and need
    // client-side progress to commit. Sequential await can starve the actual winner.
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
        },
    );
    drop(a);
    drop(b);
    let after = snapshot(&f).await;
    settings(&f, false).await;
    assert!(
        queued,
        "both independent Session writer futures must reach the real scope/resource lock chain"
    );
    let results = [ra, rb];
    let winners: Vec<usize> = results
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.is_ok().then_some(i))
        .collect();
    assert_eq!(winners.len(), 1);
    let winner = winners[0];
    let loser = 1 - winner;
    assert_eq!(
        results[loser],
        Err(SessionFileMetadataError::Upload(
            FileUploadStoreError::QuotaExceeded
        ))
    );
    let expected_owner = if winner == 0 {
        f.auth.learner.principal.reference
    } else {
        f.auth.manager.principal.reference
    };
    let expected_account = if winner == 0 {
        f.auth.learner.account_id
    } else {
        f.auth.manager.account_id
    };
    let receipt = results[winner].as_ref().unwrap();
    assert_eq!(receipt.upload_id(), upload(winner as u128 + 2));
    assert_eq!(receipt.owner(), expected_owner);
    assert_eq!(receipt.account_id(), expected_account);
    assert_eq!(receipt.manifest(), if winner == 0 { &m2 } else { &m3 });
    let token = if winner == 0 {
        &f.auth.learner_token
    } else {
        &f.auth.manager_token
    };
    assert_eq!(
        get_upload(&f, token, winner as u128 + 2).await.unwrap(),
        Some(receipt.clone())
    );
    let loser_token = if loser == 0 {
        &f.auth.learner_token
    } else {
        &f.auth.manager_token
    };
    assert_eq!(
        get_upload(&f, loser_token, loser as u128 + 2)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        get_upload(&f, &f.auth.manager_token, 1).await.unwrap(),
        Some(old)
    );
    assert_eq!(after.rows[0], before.rows[0]);
    assert_eq!(after.rows[2], before.rows[2]);
    assert_eq!(after.oids, before.oids);
    assert_eq!(
        after.rows[1].iter().find(|(n, _)| n == UPLOAD_MARKER),
        before.rows[1].iter().find(|(n, _)| n == UPLOAD_MARKER)
    );
    let old_rows = &before.rows[1]
        .iter()
        .find(|(n, _)| n == UPLOAD_ROWS)
        .unwrap()
        .1;
    let new_rows = &after.rows[1]
        .iter()
        .find(|(n, _)| n == UPLOAD_ROWS)
        .unwrap()
        .1;
    assert_eq!(new_rows.len(), old_rows.len() + 1);
    assert!(old_rows.iter().all(|r| new_rows.contains(r)));
    assert_eq!(
        reserve(&f, token, winner as u128 + 2, receipt.manifest()).await,
        Err(SessionFileMetadataError::Upload(
            FileUploadStoreError::Conflict
        )),
        "duplicate wins over exhausted quota"
    );
    f.pool_restored().await;
    f.cleanup().await;

    // Configured source READ ONLY is not overridden, even though the metadata command is called
    // reserve. This guard also uses row locks for GET, so both operations fail without writes.
    let f = Fixture::ready().await;
    configure(&f, true).await;
    let before = snapshot(&f).await;
    let read = get_upload(&f, &f.auth.learner_token, 99).await;
    let public = get_publication(&f, &f.auth.learner_token, 99).await;
    let write = reserve(&f, &f.auth.learner_token, 1, &manifest(17, 1)).await;
    assert!(read.is_err() && public.is_err() && write.is_err());
    assert_eq!(snapshot(&f).await, before);
    settings(&f, true).await;
    configure(&f, false).await;
    reserve(&f, &f.auth.learner_token, 1, &manifest(17, 1))
        .await
        .unwrap();
    settings(&f, false).await;
    f.pool_restored().await;
    f.cleanup().await;
}
