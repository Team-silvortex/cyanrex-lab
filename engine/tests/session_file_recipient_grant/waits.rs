use crate::{fixture::*, support::*, *};
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
async fn postgres_session_recipient_row_waits_recheck_expiry_cancel_and_fence_history_append() {
    for table in [MARKER, HEADS, CHANGES] {
        checked(Fixture::ready().await, move |f| {
            Box::pin(async move {
                let original = f.issue(1, f.recipient()).await;
                let mut held = holder(f, table).await;
                let pid = backend(&mut held).await;
                expire_soon(f).await;
                let before = f.snapshot().await;
                let mut pending = Box::pin(read_direct(f, &f.auth.learner_token, 1));
                let mut early = None;
                let waited = tokio::select! {
                    result=&mut pending => { early=Some(result); false },
                    yes=blocked(f,pid) => yes,
                };
                // Poll PostgreSQL's own clock; elapsed wall time alone is not dispatch/expiry evidence.
                let did_expire = waited && expired(f).await;
                held.rollback().await.unwrap();
                let result = match early {
                    Some(result) => result,
                    None => pending.as_mut().await,
                };
                drop(pending);
                let after = f.snapshot().await;
                refresh_expiry(f).await;
                let healthy = read(f, &f.auth.learner_token, 1).await;
                drain(f).await;
                assert!(
                    waited && did_expire,
                    "real selected {table} blocker must outlive DB Session expiry"
                );
                assert_eq!(
                    result,
                    Err(SessionFileRecipientGrantError::Session(
                        SessionCommandError::InvalidSession
                    )),
                    "initial authorization must not survive the later selected-row wait"
                );
                assert_eq!(after, before);
                assert_eq!(healthy.unwrap(), Some(original));
            })
        })
        .await;

        checked(Fixture::ready().await, move |f| {
            Box::pin(async move {
                let original = f.issue(1, f.recipient()).await;
                let before = f.snapshot().await;
                let mut held = holder(f, table).await;
                let pid = backend(&mut held).await;
                let mut pending = Box::pin(read_direct(f, &f.auth.learner_token, 1));
                let mut early = None;
                let waited = tokio::select! {
                    result=&mut pending => { early=Some(result); false },
                    yes=blocked(f,pid) => yes,
                };
                drop(pending); // Cancel only after an independently observed actual row wait.
                held.rollback().await.unwrap();
                drain(f).await;
                let after = f.snapshot().await;
                let healthy = read(f, &f.auth.learner_token, 1).await;
                assert!(
                    waited && early.is_none(),
                    "cancel must occur during the actual {table} wait"
                );
                assert_eq!(after, before);
                assert_eq!(healthy.unwrap(), Some(original));
            })
        })
        .await;
    }

    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let original = f.issue(1, f.recipient()).await;
            let before = f.snapshot().await;
            let mut held = holder(f, CHANGES).await;
            let holder_pid = backend(&mut held).await;
            let mut pending = Box::pin(read_direct(f, &f.auth.learner_token, 1));
            let mut early = None;
            let waited = tokio::select! {
                result=&mut pending => { early=Some(result); false },
                yes=blocked(f,holder_pid) => yes,
            };
            let reader = query(
                "SELECT pid FROM pg_stat_activity WHERE application_name=$1
            AND state='active' AND wait_event_type='Lock' AND $2=ANY(pg_blocking_pids(pid))",
            )
            .bind(&f.auth.base.schema)
            .bind(holder_pid)
            .fetch_optional(&f.auth.base.admin)
            .await
            .unwrap()
            .map(|row| row.get::<i32, _>("pid"));
            let Some(reader) = reader else {
                held.rollback().await.unwrap();
                let result = match early {
                    Some(result) => result,
                    None => pending.as_mut().await,
                };
                drop(pending);
                drain(f).await;
                panic!("selected history did not reach the precise blocker: {result:?}");
            };
            let mut writer = f.auth.base.admin.begin().await.unwrap();
            let writer_pid = backend(&mut writer).await;
            let sql = format!(
                "INSERT INTO {}.{CHANGES}(grant_id,revision,actor_id,actor_account_id,
            not_before,expires_at,recorded_at,grant_payload)
            SELECT grant_id,2,issuer_id,issuer_account_id,not_before,expires_at,
                floor(extract(epoch FROM clock_timestamp()))::bigint,grant_payload
            FROM {}.{HEADS} WHERE grant_id=$1",
                f.schema, f.schema
            );
            let mut append = Box::pin(query(&sql).bind(grant(1).as_uuid()).execute(&mut *writer));
            let mut early_append = None;
            let writer_waited = tokio::select! {
                result=&mut append => { early_append=Some(result); false },
                yes=pid_blocked_by(f,writer_pid,reader) => yes,
            };
            let during_wait = f.snapshot().await;
            held.rollback().await.unwrap();
            let observed = match early {
                Some(result) => result,
                None => pending.as_mut().await,
            };
            drop(pending);
            let appended = match early_append {
                Some(result) => result,
                None => append.as_mut().await,
            };
            drop(append);
            let affected = appended.unwrap().rows_affected();
            // Freeze the writer-only delta before its COMMIT, never derive expected data from the
            // completed reader. The reader's SHARE fence ends only with its own final checks/COMMIT.
            let injected = snapshot_in_tx(f, &mut writer).await;
            writer.commit().await.unwrap();
            let after = f.snapshot().await;
            let rejected = read(f, &f.auth.learner_token, 1).await;
            let removed = query(&format!(
                "DELETE FROM {}.{CHANGES} WHERE grant_id=$1 AND revision=2",
                f.schema
            ))
            .bind(grant(1).as_uuid())
            .execute(&f.auth.base.admin)
            .await
            .unwrap()
            .rows_affected();
            let healthy = read(f, &f.auth.learner_token, 1).await;
            drain(f).await;
            assert!(
                waited && writer_waited,
                "the exact live reader must block the independent append backend"
            );
            assert_eq!(
                during_wait, before,
                "no phantom is present while the writer is actually blocked"
            );
            assert_eq!(observed.unwrap(), Some(original.clone()));
            assert_eq!(affected, 1);
            assert_eq!(after, injected);
            assert_eq!(
                rejected,
                Err(SessionFileRecipientGrantError::Grant(
                    FileRecipientGrantStoreError::InvalidRecord
                )),
                "later revision cannot be adopted or silently omitted from a fresh selected read"
            );
            assert_eq!(removed, 1);
            assert_eq!(healthy.unwrap(), Some(original));
        })
    })
    .await;
}
