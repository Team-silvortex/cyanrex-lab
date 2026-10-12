use super::{fixture::*, support::*, *};
use std::time::Duration;

async fn expired(f: &Fixture, grant: bool) -> bool {
    tokio::time::timeout(Duration::from_secs(4),async{loop{let yes:bool=if grant {
        query(&format!("SELECT expires_at<=floor(extract(epoch FROM clock_timestamp()))::bigint AS yes FROM {}.{} WHERE grant_id=$1",f.grants.schema,grant_fixture::HEADS)).bind(grant_fixture::grant(1).as_uuid()).fetch_one(&f.grants.auth.base.admin).await.unwrap().get("yes")
    } else {query(&format!("SELECT expires_at<=clock_timestamp() AS yes FROM {}.sessions WHERE token=$1",f.grants.auth.base.schema)).bind(source_fixture::token_hash(&f.grants.auth.learner_token)).fetch_one(&f.grants.auth.base.admin).await.unwrap().get("yes")};if yes{return true;}tokio::task::yield_now().await;}}).await.unwrap_or(false)
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_publication_real_final_marker_wait_rechecks_session_and_grant_window(
) {
    for grant_expiry in [false, true] {
        let f = Fixture::ready(17).await;
        if grant_expiry {
            let now = query("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint AS n")
                .fetch_one(&f.grants.auth.base.admin)
                .await
                .unwrap()
                .get::<i64, _>("n") as u64;
            f.grants
                .store
                .issue(
                    grant_fixture::grant(1),
                    f.grants.issuer(),
                    f.actor(),
                    FileGrantValidity::new(0, now + 2).unwrap(),
                )
                .await
                .unwrap();
        } else {
            f.grants.issue(1, f.actor()).await;
        }
        let mut a = prepare(&f, 1).await;
        if !grant_expiry {
            sql(&f,&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+INTERVAL '1.5 seconds' WHERE token='{}'",f.grants.auth.base.schema,source_fixture::token_hash(&f.grants.auth.learner_token))).await;
        }
        let before = f.snapshot().await;
        let mut holder = marker_holder(&f, false).await;
        let pid = backend(&mut holder).await;
        let mut pending = Box::pin(publish(&f, &mut a));
        let mut early = None;
        let waited = tokio::select! {r=&mut pending=>{early=Some(r);false},b=blocked(&f,pid)=>b};
        let elapsed = waited && expired(&f, grant_expiry).await;
        holder.rollback().await.unwrap();
        let result = match early {
            Some(r) => r,
            None => pending.as_mut().await,
        };
        drop(pending);
        let state = a.state();
        let after = f.snapshot().await;
        if !grant_expiry {
            sql(&f,&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+INTERVAL '12 hours' WHERE token='{}'",f.grants.auth.base.schema,source_fixture::token_hash(&f.grants.auth.learner_token))).await;
        }
        f.grants.issue(2, f.actor()).await;
        let mut fresh = prepare(&f, 1).await;
        let healthy_result = publish_as(
            &f,
            &f.grants.auth.learner_token,
            &f.workspace,
            2,
            &mut fresh,
        )
        .await;
        f.cleanup().await;
        assert!(
            waited && elapsed,
            "real final target SQL lock spans actual clock expiry"
        );
        assert_eq!(
            result.err(),
            Some(if grant_expiry {
                SessionFilePublishingError::PermissionDenied
            } else {
                session_error(SessionCommandError::InvalidSession)
            })
        );
        assert_eq!(state, FileUploadPublicationState::Unknown);
        assert_eq!(after, before);
        assert!(healthy_result.is_ok());
    }
    let f = Fixture::ready(17).await;
    f.grants.issue(1, f.actor()).await;
    let mut a = prepare(&f, 1).await;
    let old_id = query(&format!(
        "SELECT installation_id FROM {}.{PUBLICATION_MARKER}",
        f.publication_schema
    ))
    .fetch_one(&f.grants.auth.base.admin)
    .await
    .unwrap()
    .get::<Uuid, _>("installation_id");
    let mut holder = marker_holder(&f, true).await;
    let pid = backend(&mut holder).await;
    let mut pending = Box::pin(publish(&f, &mut a));
    let mut early = None;
    let waited = tokio::select! {r=&mut pending=>{early=Some(r);false},b=blocked(&f,pid)=>b};
    query(&format!(
        "UPDATE {}.{PUBLICATION_MARKER} SET installation_id=$1",
        f.publication_schema
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
    let after = f.snapshot().await;
    query(&format!(
        "UPDATE {}.{PUBLICATION_MARKER} SET installation_id=$1",
        f.publication_schema
    ))
    .bind(old_id)
    .execute(&f.grants.auth.base.admin)
    .await
    .unwrap();
    healthy(&f).await;
    f.cleanup().await;
    assert!(waited);
    assert!(result.is_err());
    assert_eq!(
        after, injected,
        "fresh target incarnation read cannot revert the independently committed mutation"
    );
}
