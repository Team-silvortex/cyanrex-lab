use super::sql_fixture::*;
use super::*;

async fn waiting_step(final_task: bool, expire: bool) {
    let f = Fixture::ready().await;
    let items = if final_task {
        vec![("a", "text", "confirmed input")]
    } else {
        vec![
            ("a", "text", "confirmed input"),
            ("b", "text", "waiting input"),
        ]
    };
    let mut attempt = prepare(&f, &values()[0], &items);
    attempt.advance(&f.auth.learner_token).await.unwrap();
    let first = attempt.confirmed_artifacts().to_vec();
    let target = attempt.allocated_artifacts().get(1).cloned();
    let state = if final_task {
        uncertain_task(attempt.task_ref())
    } else {
        uncertain_artifact(1, target.clone().unwrap())
    };
    let before = observed(&f).await;
    let mut holder = if final_task {
        f.pause_task_event().await
    } else {
        pause_artifact_event(&f).await
    };
    let blocker = backend(&mut holder).await;
    if expire {
        query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
            .bind(source_fixture::token_hash(&f.auth.learner_token))
            .execute(&f.auth.base.pool).await.unwrap();
    }
    let mut pending = Box::pin(attempt.advance(&f.auth.learner_token));
    let mut early = None;
    let waited = tokio::select! {
        result = &mut pending => { early = Some(result); false },
        waiter = blocked_by(&f, blocker) => waiter.is_some(),
    };
    let expired = if expire && waited {
        wait_expired(&f).await
    } else {
        false
    };
    let result = if expire {
        holder.rollback().await.unwrap();
        let result = match early {
            Some(result) => result,
            None => pending.as_mut().await,
        };
        drop(pending);
        Some(result)
    } else {
        drop(pending);
        holder.rollback().await.unwrap();
        None
    };
    f.auth.base.drain().await;
    f.assert_pool_restored().await;
    assert!(waited, "must observe the exact {final_task} command blocked in PostgreSQL before cancellation/expiry");
    if expire {
        assert!(
            expired,
            "expiry is observed by database clock while the dispatched request remains alive"
        );
        let error = if final_task {
            SessionDraftPublicationError::Task(SessionTaskError::Session(
                SessionCommandError::InvalidSession,
            ))
        } else {
            SessionDraftPublicationError::Artifact(SessionArtifactError::Session(
                SessionCommandError::InvalidSession,
            ))
        };
        assert_eq!(result, Some(Err(error)));
    }
    assert_eq!(attempt.state(), &state);
    assert_eq!(attempt.confirmed_artifacts(), first);
    assert!(attempt.confirmed_task().is_none());
    assert_eq!(
        fs::read(f.blob(&first[0].reference)).unwrap(),
        b"confirmed input"
    );
    let after = observed(&f).await;
    assert_eq!(
        after.rows, before.rows,
        "all committed resource rows are preserved"
    );
    if final_task {
        assert_eq!(after.files, before.files);
        f.sql_task("DROP TRIGGER pause_content_event ON collaboration_task_outbox")
            .await;
    } else {
        let target = target.unwrap();
        let mut files = before.files;
        files.push((
            f.blob(&target)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            b"waiting input".to_vec(),
        ));
        files.sort();
        assert_eq!(
            after.files, files,
            "the unknown exact-target blob remains, without any other file mutation"
        );
        f.sql_artifact("DROP TRIGGER pause_draft_event ON collaboration_artifact_outbox")
            .await;
    }
    let before_retry = observed(&f).await;
    assert_eq!(
        attempt.advance(&f.auth.learner_token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
    assert_eq!(observed(&f).await, before_retry);
    drop(attempt);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_publication_observed_artifact_and_task_waits_keep_cancellation_unconfirmed(
) {
    for final_task in [false, true] {
        waiting_step(final_task, false).await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_publication_observed_artifact_and_task_waits_recheck_database_session_expiry(
) {
    for final_task in [false, true] {
        waiting_step(final_task, true).await;
    }
}
