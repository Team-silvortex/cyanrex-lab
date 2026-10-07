use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_cancelled_artifact_wait_keeps_uncertain_target_and_blob() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, &[("a", "text", "first"), ("b", "text", "second")]);
    attempt.advance(&f.auth.learner_token).await.unwrap();
    let first = attempt.confirmed_artifacts().to_vec();
    let next = attempt.allocated_artifacts()[1].clone();
    let mut holder = pause_artifact_event(&f).await;
    let blocker = backend(&mut holder).await;
    let mut pending = Box::pin(attempt.advance(&f.auth.learner_token));
    let waited = tokio::select! {
        _ = &mut pending => false,
        waiter = blocked_by(&f, blocker) => waiter.is_some(),
    };
    // Drop only the borrowed future; the caller still owns the progress ledger.
    drop(pending);
    let state = attempt.state().clone();
    holder.rollback().await.unwrap();
    f.auth.base.drain().await;
    f.assert_pool_restored().await;
    let confirmed = attempt.confirmed_artifacts().to_vec();
    let no_task = attempt.confirmed_task().is_none();
    let after = counts(&f).await;
    f.sql_artifact("DROP TRIGGER pause_draft_event ON collaboration_artifact_outbox")
        .await;
    let retry = attempt.advance(&f.auth.learner_token).await;
    let after_retry = counts(&f).await;
    let first_read = f
        .artifacts
        .read(&first[0].reference, first[0].owner)
        .await
        .unwrap();
    f.cleanup().await;
    assert!(
        waited,
        "advance must reach the exact outbox blocker before cancellation"
    );
    assert_eq!(state, uncertain_artifact(1, next));
    assert_eq!(confirmed, first);
    assert!(no_task);
    assert_eq!(first_read.unwrap().bytes, b"first");
    assert_eq!(after, [1, 1, 1, 0, 0, 2]);
    assert_eq!(retry, Err(SessionDraftPublicationError::Stopped));
    assert_eq!(after_retry, after);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_cancelled_task_wait_preserves_inputs_without_confirmation() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, &[("a", "text", "retained")]);
    attempt.advance(&f.auth.learner_token).await.unwrap();
    let artifacts = attempt.confirmed_artifacts().to_vec();
    let target = attempt.task_ref();
    let mut holder = f.pause_task_event().await;
    let blocker = backend(&mut holder).await;
    let mut pending = Box::pin(attempt.advance(&f.auth.learner_token));
    let waited = tokio::select! {
        _ = &mut pending => false,
        waiter = blocked_by(&f, blocker) => waiter.is_some(),
    };
    drop(pending);
    let state = attempt.state().clone();
    holder.rollback().await.unwrap();
    f.auth.base.drain().await;
    f.assert_pool_restored().await;
    let confirmed = attempt.confirmed_artifacts().to_vec();
    let no_task = attempt.confirmed_task().is_none();
    let after = counts(&f).await;
    f.sql_task("DROP TRIGGER pause_content_event ON collaboration_task_outbox")
        .await;
    let retry = attempt.advance(&f.auth.learner_token).await;
    let after_retry = counts(&f).await;
    f.cleanup().await;
    assert!(
        waited,
        "advance must reach the exact task event blocker before cancellation"
    );
    assert_eq!(state, uncertain_task(target));
    assert_eq!(confirmed, artifacts);
    assert!(no_task);
    assert_eq!(after, [1, 1, 1, 0, 0, 1]);
    assert_eq!(retry, Err(SessionDraftPublicationError::Stopped));
    assert_eq!(after_retry, after);
}
