use super::cleanup::checked;
use super::{fixture::*, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_cancelled_real_recipient_deferred_wait_observes_rollback_without_retry_authority()
{
    for revoke_case in [false, true] {
        checked(ready(8).await, |f| {
            Box::pin(async move {
                if revoke_case {
                    issue(&f, 1, account(1), account(2), account(3), validity())
                        .await
                        .unwrap();
                }
                install_trigger(
                    &f,
                    CHANGES,
                    "INSERT",
                    "PERFORM pg_advisory_xact_lock(82739,62423); RETURN NEW;",
                    true,
                )
                .await;
                let before = f.snapshot().await;
                let mut holder = f.admin.begin().await.unwrap();
                let pid = backend(&mut holder).await;
                query("SELECT pg_advisory_xact_lock(82739,62423)")
                    .execute(&mut *holder)
                    .await
                    .unwrap();
                let mut pending = Box::pin(async {
                    if revoke_case {
                        f.store.revoke(grant(1), account(9), revision(1)).await
                    } else {
                        f.store
                            .issue(
                                grant(1),
                                account(1),
                                file(1),
                                account(2),
                                account(3),
                                validity(),
                            )
                            .await
                    }
                });
                let waited = tokio::select! {_=&mut pending=>false,w=blocked(&f,pid)=>w};
                let uncommitted = f.snapshot().await;
                let fired = hits(&f).await;
                drop(pending);
                holder.commit().await.unwrap();
                drain(&f).await;
                let after = f.snapshot().await;
                drop_trigger(&f, CHANGES).await;
                if revoke_case {
                    revoke(&f, 1, account(9), revision(1)).await.unwrap();
                } else {
                    issue(&f, 1, account(1), account(2), account(3), validity())
                        .await
                        .unwrap();
                }
                assert!(waited && fired);
                assert_eq!(uncommitted, before);
                assert_eq!(after, before);
            })
        })
        .await;
    }
    // Actual cancellation is during SET CONSTRAINTS ALL IMMEDIATE, BEFORE COMMIT.
    // Rollback is independently observed after all locks drain; this is not a general
    // lost-ACK simulation and does not authorize hidden retries or adoption.
}
