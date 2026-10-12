use super::support::*;
use crate::{fixture::*, *};
use std::time::Duration;

async fn pid_blocked_by(f: &Fixture, waiter: i32, blocker: i32) -> bool {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let yes: bool = query("SELECT $1=ANY(pg_blocking_pids($2)) AS yes")
                .bind(blocker)
                .bind(waiter)
                .fetch_one(&f.auth.base.admin)
                .await
                .unwrap()
                .get("yes");
            if yes {
                return true;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or(false)
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_publisher_grant_shared_row_waits_recheck_expiry_cancel_and_prevent_history_append(
) {
    for table in [MARKER, HEADS, CHANGES] {
        let f = Fixture::ready().await;
        let original = f.issue(1, f.subject()).await;
        let mut held = holder(&f, table).await;
        let pid = backend(&mut held).await;
        expire_soon(&f).await;
        let before = f.snapshot().await;
        let mut pending = Box::pin(read_direct(&f, &f.auth.learner_token, 1));
        let mut early = None;
        let waited =
            tokio::select! {r=&mut pending=>{early=Some(r);false}, yes=blocked(&f,pid)=>yes};
        let elapsed = waited && expired(&f).await;
        held.rollback().await.unwrap();
        let result = match early {
            Some(r) => r,
            None => pending.as_mut().await,
        };
        drop(pending);
        let after = f.snapshot().await;
        refresh_expiry(&f).await;
        let healthy = read(&f, &f.auth.learner_token, 1).await;
        drain(&f).await;
        f.cleanup().await;
        assert!(
            waited && elapsed,
            "GET must really wait on the selected {table} row until DB-clock Session expiry"
        );
        assert_eq!(
            result,
            Err(SessionFilePublisherGrantError::Session(
                SessionCommandError::InvalidSession
            ))
        );
        assert_eq!(after, before);
        assert_eq!(healthy.unwrap(), Some(original));

        let f = Fixture::ready().await;
        let original = f.issue(1, f.subject()).await;
        let before = f.snapshot().await;
        let mut held = holder(&f, table).await;
        let pid = backend(&mut held).await;
        let mut pending = Box::pin(read_direct(&f, &f.auth.learner_token, 1));
        let waited = tokio::select! {_=&mut pending=>false, yes=blocked(&f,pid)=>yes};
        drop(pending);
        held.rollback().await.unwrap();
        drain(&f).await;
        let after = f.snapshot().await;
        let healthy = read(&f, &f.auth.learner_token, 1).await;
        f.cleanup().await;
        assert!(
            waited,
            "cancel only after exact {table} blocker is independently observed"
        );
        assert_eq!(
            after, before,
            "cancelled reads must not leave writes or new object identities"
        );
        assert_eq!(healthy.unwrap(), Some(original));
    }

    // An existing history-row lock alone cannot block a new revision. The borrowed reader's
    // table SHARE locks must additionally keep an append writer out through source COMMIT.
    let f = Fixture::ready().await;
    let original = f.issue(1, f.subject()).await;
    let before = f.snapshot().await;
    let mut held = holder(&f, CHANGES).await;
    let holder_pid = backend(&mut held).await;
    let mut pending = Box::pin(read_direct(&f, &f.auth.learner_token, 1));
    let waited = tokio::select! {_=&mut pending=>false, yes=blocked(&f,holder_pid)=>yes};
    let reader: i32 = query("SELECT pid FROM pg_stat_activity WHERE application_name=$1 AND $2=ANY(pg_blocking_pids(pid))")
        .bind(&f.auth.base.schema).bind(holder_pid).fetch_one(&f.auth.base.admin).await.unwrap().get("pid");
    let mut writer = f.auth.base.admin.begin().await.unwrap();
    let writer_pid = backend(&mut writer).await;
    let sql = format!("INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,not_before,expires_at,recorded_at,grant_payload) SELECT grant_id,2,issuer_id,issuer_account_id,not_before,expires_at,floor(extract(epoch FROM clock_timestamp()))::bigint,grant_payload FROM {}.{HEADS} WHERE grant_id=$1", f.schema, f.schema);
    let mut append = Box::pin(query(&sql).bind(grant(1).as_uuid()).execute(&mut *writer));
    let mut early_append = None;
    let writer_waited = tokio::select! {r=&mut append=>{early_append=Some(r);false}, yes=pid_blocked_by(&f,writer_pid,reader)=>yes};
    assert_eq!(
        f.snapshot().await,
        before,
        "blocked append has not yet created a history phantom"
    );
    held.rollback().await.unwrap();
    let observed = pending.as_mut().await;
    drop(pending);
    let appended = match early_append {
        Some(r) => r,
        None => append.as_mut().await,
    };
    drop(append);
    assert_eq!(appended.unwrap().rows_affected(), 1);
    let injected = snapshot_in_tx(&f, &mut writer).await;
    writer.commit().await.unwrap();
    assert!(
        waited && writer_waited,
        "the actual reader must block the separate append backend, not merely an unrelated lock"
    );
    assert_eq!(observed.unwrap(), Some(original.clone()));
    assert_eq!(f.snapshot().await, injected);
    assert_eq!(
        read(&f, &f.auth.learner_token, 1).await,
        Err(SessionFilePublisherGrantError::Grant(
            FilePublisherGrantStoreError::InvalidRecord
        )),
        "the later corrupt revision must be rejected by a fresh read, not silently ignored"
    );
    assert_eq!(
        query(&format!(
            "DELETE FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=2",
            f.schema
        ))
        .bind(grant(1).as_uuid())
        .execute(&f.auth.base.admin)
        .await
        .unwrap()
        .rows_affected(),
        1
    );
    assert_eq!(
        read(&f, &f.auth.learner_token, 1).await.unwrap(),
        Some(original)
    );
    drain(&f).await;
    f.cleanup().await;
}
