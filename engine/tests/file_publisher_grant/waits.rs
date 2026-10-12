use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cancelled_real_publisher_grant_deferred_flush_wait_retains_no_retry_authority() {
    for revoke_case in [false, true] {
        let f = ready(8).await;
        if revoke_case {
            issue(&f, 1, account(1), account(2), validity())
                .await
                .unwrap();
        }
        install_trigger(
            &f,
            CHANGES,
            "INSERT",
            "PERFORM pg_advisory_xact_lock(82731,62417); RETURN NEW;",
            true,
        )
        .await;
        let before = snapshot(&f).await;
        let mut holder = f.admin.begin().await.unwrap();
        let pid = backend(&mut holder).await;
        query("SELECT pg_advisory_xact_lock(82731,62417)")
            .execute(&mut *holder)
            .await
            .unwrap();
        let mut pending = Box::pin(async {
            if revoke_case {
                f.store.revoke(grant(1), account(9), revision(1)).await
            } else {
                f.store
                    .issue(grant(1), account(1), account(2), validity())
                    .await
            }
        });
        let waited = tokio::select! {_=&mut pending=>false,w=blocked(&f,pid,1)=>w};
        // Independent snapshots while SET CONSTRAINTS ALL IMMEDIATE is actually blocked
        // in deferred work. This is before COMMIT, not a COMMIT/lost-ACK cancellation case.
        let uncommitted = snapshot(&f).await;
        let fired = hits(&f).await;
        drop(pending);
        holder.commit().await.unwrap();
        drain(&f).await;
        let rolled_back = snapshot(&f).await;
        drop_trigger(&f, CHANGES).await;
        // A deliberate NEW trusted call is safe here only because rollback was observed.
        // Cancellation itself confers no automatic retry authority.
        if revoke_case {
            revoke(&f, 1, account(9), revision(1)).await.unwrap();
        } else {
            issue(&f, 1, account(1), account(2), validity())
                .await
                .unwrap();
        }
        f.cleanup().await;
        assert!(waited);
        assert!(fired);
        assert_eq!(uncommitted, before);
        assert_eq!(rolled_back, before);
    }
}
