use super::support::*;
use crate::{fixture::*, *};
use std::time::Duration;
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_sharing_real_waits_recheck_expiry_and_cancel_before_commit_with_rollback_observation(
) {
    for is_revoke in [false, true] {
        checked(Fixture::ready().await, |f| {
            Box::pin(async move {
                if is_revoke {
                    issue(f, 1).await.unwrap();
                }
                expire_soon(f).await;
                let before = f.snapshot().await;
                let mut holder = marker_holder(f, &f.schema, MARKER, false).await;
                let pid = backend(&mut holder).await;
                let c = f.command(1);
                let mut pending = Box::pin(async {
                    if is_revoke {
                        f.base
                            .grants
                            .auth
                            .source
                            .revoke_session_file_recipient_grant(
                                &f.base.grants.auth.learner_token,
                                &f.workspace,
                                recipient_grant(1),
                                revision(1),
                            )
                            .await
                    } else {
                        f.base
                            .grants
                            .auth
                            .source
                            .issue_session_file_recipient_grant(
                                &f.base.grants.auth.learner_token,
                                &f.workspace,
                                &c,
                            )
                            .await
                    }
                });
                let mut early = None;
                let waited =
                    tokio::select! {r=&mut pending=>{early=Some(r);false},w=blocked(f,pid)=>w};
                let elapsed = waited && expired(f).await;
                holder.rollback().await.unwrap();
                let result = match early {
                    Some(r) => r,
                    None => pending.as_mut().await,
                };
                drop(pending);
                assert_eq!(f.snapshot().await, before);
                refresh_expiry(f).await;
                let healthy = if is_revoke {
                    revoke(f, 1, revision(1)).await
                } else {
                    issue(f, 1).await
                };
                assert!(healthy.is_ok());
                assert!(waited && elapsed);
                assert_eq!(result, Err(session(SessionCommandError::InvalidSession)));
                drain(f).await;
            })
        })
        .await;
    }
    checked(Fixture::ready().await,|f|Box::pin(async move{
        let now:i64=query("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint AS n").fetch_one(&f.base.grants.auth.base.admin).await.unwrap().get("n");let expiry=now as u64+2;
        f.base.grants.store.issue(grant_fixture::grant(8),f.base.grants.issuer(),f.actor(),FileGrantValidity::new(0,expiry).unwrap()).await.unwrap();fault(f,CHANGES,"INSERT","RETURN NEW;",false).await;let before=f.snapshot().await;let mut holder=marker_holder(f,&f.schema,MARKER,false).await;let pid=backend(&mut holder).await;let mut c=f.command(1);c.publisher_grant_id=grant_fixture::grant(8);let mut pending=Box::pin(f.base.grants.auth.source.issue_session_file_recipient_grant(&f.base.grants.auth.learner_token,&f.workspace,&c));let mut early=None;let waited=tokio::select!{r=&mut pending=>{early=Some(r);false},w=blocked(f,pid)=>w};
        let elapsed=waited&&tokio::time::timeout(Duration::from_secs(2),async{loop{let yes:bool=query("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint >= $1 AS yes").bind(expiry as i64).fetch_one(&f.base.grants.auth.base.admin).await.unwrap().get("yes");if yes{return true;}tokio::task::yield_now().await;}}).await.unwrap_or(false);
        holder.rollback().await.unwrap();let result=match early{Some(r)=>r,None=>pending.as_mut().await};drop(pending);let reached=hit(f).await;assert_eq!(f.snapshot().await,before);remove_fault(f,CHANGES).await;issue(f,1).await.unwrap();assert!(waited&&elapsed&&reached,"initial valid grant must survive a real recipient marker wait, reach tentative writes and expire before the FINAL sample");assert_eq!(result,Err(SessionFileSharingError::PermissionDenied));
    })).await;
    checked(Fixture::ready().await, |f| {
        Box::pin(async move {
            let before = f.snapshot().await;
            let mut holder = marker_holder(f, &f.schema, MARKER, true).await;
            let pid = backend(&mut holder).await;
            let c = f.command(1);
            let mut pending = Box::pin(
                f.base
                    .grants
                    .auth
                    .source
                    .issue_session_file_recipient_grant(
                        &f.base.grants.auth.learner_token,
                        &f.workspace,
                        &c,
                    ),
            );
            let mut early = None;
            let waited = tokio::select! {r=&mut pending=>{early=Some(r);false},w=blocked(f,pid)=>w};
            query(&format!(
                "UPDATE {}.{MARKER} SET installation_id=$1",
                f.schema
            ))
            .bind(Uuid::new_v4())
            .execute(&mut *holder)
            .await
            .unwrap();
            let injected = snapshot_in_tx(f, &mut holder).await;
            holder.commit().await.unwrap();
            let result = match early {
                Some(r) => r,
                None => pending.as_mut().await,
            };
            drop(pending);
            assert_eq!(f.snapshot().await, injected);
            assert_ne!(injected.recipient, before.recipient);
            query(&format!(
                "UPDATE {}.{MARKER} SET installation_id=$1",
                f.schema
            ))
            .bind(f.installation_id)
            .execute(&f.base.grants.auth.base.admin)
            .await
            .unwrap();
            issue(f, 1).await.unwrap();
            assert!(waited);
            assert_eq!(
                result,
                Err(recipient(FileRecipientGrantStoreError::IncarnationMismatch))
            );
        })
    })
    .await;
    for is_revoke in [false, true] {
        checked(Fixture::ready().await, |f| {
            Box::pin(async move {
                if is_revoke {
                    issue(f, 1).await.unwrap();
                }
                fault(
                    f,
                    CHANGES,
                    "INSERT",
                    "PERFORM pg_advisory_xact_lock(82743,62429); RETURN NEW;",
                    true,
                )
                .await;
                let before = f.snapshot().await;
                let mut holder = f.base.grants.auth.base.admin.begin().await.unwrap();
                let pid = backend(&mut holder).await;
                query("SELECT pg_advisory_xact_lock(82743,62429)")
                    .execute(&mut *holder)
                    .await
                    .unwrap();
                let c = f.command(1);
                let mut pending = Box::pin(async {
                    if is_revoke {
                        f.base
                            .grants
                            .auth
                            .source
                            .revoke_session_file_recipient_grant(
                                &f.base.grants.auth.learner_token,
                                &f.workspace,
                                recipient_grant(1),
                                revision(1),
                            )
                            .await
                    } else {
                        f.base
                            .grants
                            .auth
                            .source
                            .issue_session_file_recipient_grant(
                                &f.base.grants.auth.learner_token,
                                &f.workspace,
                                &c,
                            )
                            .await
                    }
                });
                let waited = tokio::select! {_=&mut pending=>false,w=blocked(f,pid)=>w};
                let uncommitted = f.snapshot().await;
                let reached = hit(f).await;
                drop(pending);
                holder.commit().await.unwrap();
                drain(f).await;
                let rolled_back = f.snapshot().await;
                remove_fault(f, CHANGES).await;
                let healthy = if is_revoke {
                    revoke(f, 1, revision(1)).await
                } else {
                    issue(f, 1).await
                };
                assert!(healthy.is_ok());
                assert!(waited && reached);
                assert_eq!(uncommitted, before);
                assert_eq!(rolled_back, before);
            })
        })
        .await;
    }
    // Cancellation is proved at SET CONSTRAINTS ALL IMMEDIATE BEFORE COMMIT, followed by
    // independent rollback/lock-drain observation. This is not arbitrary lost acknowledgement,
    // an automatic retry policy or an absence-of-grant inference from an unavailable query.
}
