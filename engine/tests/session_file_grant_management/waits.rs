use super::{
    fixture::*,
    publisher_fixture::{grant, validity, CHANGES, MARKER},
    support::*,
    *,
};
use std::time::Duration;

pub(crate) async fn marker_holder(f: &Fixture) -> Tx<'_> {
    let mut tx = f.base.auth.base.admin.begin().await.unwrap();
    query(&format!(
        "SELECT singleton FROM {}.{MARKER} FOR UPDATE",
        f.base.schema
    ))
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    tx
}
async fn expired(f: &Fixture) -> bool {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let yes: bool = query(&format!(
                "SELECT expires_at<=clock_timestamp() AS yes FROM {}.sessions WHERE token=$1",
                f.base.auth.base.schema
            ))
            .bind(source_fixture::token_hash(&f.base.auth.learner_token))
            .fetch_one(&f.base.auth.base.admin)
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
async fn postgres_session_file_grant_real_waits_recheck_expiry_logout_fresh_marker_and_cancelled_writes(
) {
    for revoke_case in [false, true] {
        let f = Fixture::ready().await;
        if revoke_case {
            issue(&f, 1, f.target()).await.unwrap();
        }
        query("UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '1.3 seconds' WHERE token=$1").bind(source_fixture::token_hash(&f.base.auth.learner_token)).execute(&f.base.auth.base.pool).await.unwrap();
        let before = f.base.snapshot().await;
        let mut holder = marker_holder(&f).await;
        let pid = backend(&mut holder).await;
        let mut pending = Box::pin(async {
            if revoke_case {
                f.base
                    .auth
                    .source
                    .revoke_session_file_publisher_grant(
                        &f.base.auth.learner_token,
                        &f.management,
                        grant(1),
                        revision(1),
                    )
                    .await
            } else {
                f.base
                    .auth
                    .source
                    .issue_session_file_publisher_grant(
                        &f.base.auth.learner_token,
                        &f.management,
                        grant(1),
                        f.target(),
                        validity(),
                    )
                    .await
            }
        });
        let mut early = None;
        let waited = tokio::select! {r=&mut pending=>{early=Some(r);false},w=blocked(&f,pid)=>w};
        let elapsed = waited && expired(&f).await;
        holder.rollback().await.unwrap();
        let result = match early {
            Some(r) => r,
            None => pending.as_mut().await,
        };
        drop(pending);
        let after = f.base.snapshot().await;
        query(
            "UPDATE sessions SET expires_at=clock_timestamp()+INTERVAL '12 hours' WHERE token=$1",
        )
        .bind(source_fixture::token_hash(&f.base.auth.learner_token))
        .execute(&f.base.auth.base.pool)
        .await
        .unwrap();
        let healthy = if revoke_case {
            revoke(&f, 1, revision(1)).await
        } else {
            issue(&f, 1, f.target()).await
        };
        f.cleanup().await;
        assert!(waited && elapsed);
        assert_eq!(after, before);
        assert_eq!(
            result,
            Err(session_error(SessionCommandError::InvalidSession))
        );
        assert!(healthy.is_ok());
    }
    let f = Fixture::ready().await;
    let mut holder = f.base.auth.base.admin.begin().await.unwrap();
    let pid = backend(&mut holder).await;
    query(&format!(
        "SELECT token FROM {}.sessions WHERE token=$1 FOR UPDATE",
        f.base.auth.base.schema
    ))
    .bind(source_fixture::token_hash(&f.base.auth.learner_token))
    .fetch_one(&mut *holder)
    .await
    .unwrap();
    let mut pending = Box::pin(f.base.auth.source.issue_session_file_publisher_grant(
        &f.base.auth.learner_token,
        &f.management,
        grant(1),
        f.target(),
        validity(),
    ));
    let mut early = None;
    let waited = tokio::select! {r=&mut pending=>{early=Some(r);false},w=blocked(&f,pid)=>w};
    query(&format!(
        "DELETE FROM {}.sessions WHERE token=$1",
        f.base.auth.base.schema
    ))
    .bind(source_fixture::token_hash(&f.base.auth.learner_token))
    .execute(&mut *holder)
    .await
    .unwrap();
    let injected = snapshot_in_tx(&f, &mut holder).await;
    holder.commit().await.unwrap();
    let result = match early {
        Some(r) => r,
        None => pending.as_mut().await,
    };
    drop(pending);
    let after = f.base.snapshot().await;
    let token = f
        .base
        .auth
        .base
        .seed_session(&DurableAccountRef {
            authority_id: authority(),
            username: f.base.auth.learner.binding.username.clone(),
            account_id: f.base.auth.learner.account_id,
        })
        .await;
    // Refresh only the fixture token: no implementation retries or fallback Sessions.
    let healthy = issue_as(&f, &token.token, &f.management, 1, f.target(), validity()).await;
    f.cleanup().await;
    assert!(waited);
    assert_eq!(after, injected);
    assert_eq!(
        result,
        Err(session_error(SessionCommandError::InvalidSession))
    );
    assert!(healthy.is_ok());

    let f = Fixture::ready().await;
    let mut holder = marker_holder(&f).await;
    // Admit the writer table lock BEFORE the Session command's SRE fence. Otherwise
    // a later UPDATE would upgrade behind that fence while the command waits on our row.
    query(&format!(
        "LOCK TABLE {}.{MARKER} IN ROW EXCLUSIVE MODE",
        f.base.schema
    ))
    .execute(&mut *holder)
    .await
    .unwrap();
    let pid = backend(&mut holder).await;
    let mut pending = Box::pin(f.base.auth.source.issue_session_file_publisher_grant(
        &f.base.auth.learner_token,
        &f.management,
        grant(1),
        f.target(),
        validity(),
    ));
    let mut early = None;
    let waited = tokio::select! {r=&mut pending=>{early=Some(r);false},w=blocked(&f,pid)=>w};
    query(&format!(
        "UPDATE {}.{MARKER} SET installation_id=$1",
        f.base.schema
    ))
    .bind(Uuid::new_v4())
    .execute(&mut *holder)
    .await
    .unwrap();
    let injected = snapshot_in_tx(&f, &mut holder).await;
    holder.commit().await.unwrap();
    let result = match early {
        Some(r) => r,
        None => pending.as_mut().await,
    };
    drop(pending);
    assert_eq!(f.base.snapshot().await, injected);
    query(&format!(
        "UPDATE {}.{MARKER} SET installation_id=$1",
        f.base.schema
    ))
    .bind(f.base.installation_id)
    .execute(&f.base.pool)
    .await
    .unwrap();
    let healthy = issue(&f, 1, f.target()).await;
    f.cleanup().await;
    assert!(waited);
    assert_eq!(
        result,
        Err(grant_error(
            FilePublisherGrantStoreError::IncarnationMismatch
        ))
    );
    assert!(healthy.is_ok());

    super::wait_logout::command_admitted_before_real_logout().await;

    for revoke_case in [false, true] {
        let f = Fixture::ready().await;
        if revoke_case {
            issue(&f, 1, f.target()).await.unwrap();
        }
        fault(
            &f,
            CHANGES,
            "INSERT",
            "PERFORM pg_advisory_xact_lock(79183,73481); RETURN NEW;",
            true,
        )
        .await;
        let before = f.base.snapshot().await;
        let mut holder = f.base.auth.base.admin.begin().await.unwrap();
        let pid = backend(&mut holder).await;
        query("SELECT pg_advisory_xact_lock(79183,73481)")
            .execute(&mut *holder)
            .await
            .unwrap();
        let mut pending = Box::pin(async {
            if revoke_case {
                f.base
                    .auth
                    .source
                    .revoke_session_file_publisher_grant(
                        &f.base.auth.learner_token,
                        &f.management,
                        grant(1),
                        revision(1),
                    )
                    .await
            } else {
                f.base
                    .auth
                    .source
                    .issue_session_file_publisher_grant(
                        &f.base.auth.learner_token,
                        &f.management,
                        grant(1),
                        f.target(),
                        validity(),
                    )
                    .await
            }
        });
        let waited = tokio::select! {_=&mut pending=>false,w=blocked(&f,pid)=>w};
        let uncommitted = f.base.snapshot().await;
        let reached = hit(&f).await;
        drop(pending);
        holder.rollback().await.unwrap();
        drain(&f).await;
        let after = f.base.snapshot().await;
        remove_fault(&f, CHANGES).await;
        // Only this explicit independent observation establishes rollback. A cancelled or
        // lost-ACK call in general cannot safely infer missing history or authorize retry.
        let healthy = if revoke_case {
            revoke(&f, 1, revision(1)).await
        } else {
            issue(&f, 1, f.target()).await
        };
        f.cleanup().await;
        assert!(waited && reached);
        assert_eq!(uncommitted, before);
        assert_eq!(after, before);
        assert!(healthy.is_ok());
    }
}
