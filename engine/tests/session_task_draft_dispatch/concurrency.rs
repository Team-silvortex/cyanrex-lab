use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_cancelled_phase_a_cannot_publish_or_retry() {
    for task in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = f.journaled(if task { &[] } else { ITEMS }).await;
        let before = f.state().await;
        let mut holder = f.pause_phase("INSERT").await;
        let blocker = backend(&mut holder).await;
        let mut pending = Box::pin(attempt.advance(&f.auth.learner_token));
        let waited = tokio::select! {
            _ = &mut pending => false,
            waiter = blocked_by(&f, blocker) => waiter.is_some(),
        };
        drop(pending);
        holder.rollback().await.unwrap();
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        let hit = f.step_hit().await;
        let steps = f.steps().await;
        let effects = counts(&f).await;
        let unchanged = before == f.state().await;
        let terminal = unknown(&attempt);
        let retry = attempt.advance(&f.auth.learner_token).await;
        f.cleanup().await;
        assert!(waited && hit && unchanged && terminal);
        assert_eq!(retry, Err(stopped()));
        assert!(steps.is_empty());
        assert_eq!(effects, [0; 6]);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_cancelled_phase_b_keeps_unknown_without_resource_commit() {
    for task in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = f.journaled(if task { &[] } else { ITEMS }).await;
        if !task {
            attempt.advance(&f.auth.learner_token).await.unwrap();
        }
        let prefix = attempt.confirmed_artifacts().to_vec();
        let before = counts(&f).await;
        let mut holder = f.pause_phase("UPDATE").await;
        let blocker = backend(&mut holder).await;
        let mut pending = Box::pin(attempt.advance(&f.auth.learner_token));
        let waited = tokio::select! {
            _ = &mut pending => false,
            waiter = blocked_by(&f, blocker) => waiter.is_some(),
        };
        // A has committed the durable Unknown while B is stopped before the resource write.
        let during = f.steps().await;
        drop(pending);
        holder.rollback().await.unwrap();
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        let hit = f.step_hit().await;
        let steps = f.steps().await;
        let after = counts(&f).await;
        let terminal = unknown(&attempt);
        let retained = attempt.confirmed_artifacts() == prefix;
        let retry = attempt.advance(&f.auth.learner_token).await;
        f.cleanup().await;
        assert!(waited && hit && terminal && retained);
        assert_eq!(retry, Err(stopped()));
        assert_eq!(steps, during);
        assert_eq!(steps.len(), if task { 1 } else { 2 });
        assert!(!steps.last().unwrap().2);
        assert_eq!(before, after);
        // This controlled pre-write cancellation establishes rollback only for this fixture.
    }
}

fn expired(error: &SessionDraftDispatchError) -> bool {
    matches!(
        error,
        SessionDraftDispatchError::Intent(SessionDraftIntentError::Session(
            SessionCommandError::InvalidSession
        )) | SessionDraftDispatchError::Artifact(SessionArtifactError::Session(
            SessionCommandError::InvalidSession
        )) | SessionDraftDispatchError::Task(SessionTaskError::Session(
            SessionCommandError::InvalidSession
        ))
    )
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_waiting_phase_a_and_b_recheck_database_expiry() {
    for phase in ["INSERT", "UPDATE"] {
        for task in [false, true] {
            let f = Fixture::ready().await;
            let mut attempt = f.journaled(if task { &[] } else { ITEMS }).await;
            let mut holder = f.pause_phase(phase).await;
            let blocker = backend(&mut holder).await;
            query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
                .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
            let mut pending = Box::pin(attempt.advance(&f.auth.learner_token));
            let mut early = None;
            let waited = tokio::select! {
                result = &mut pending => { early = Some(result); false },
                waiter = blocked_by(&f, blocker) => waiter.is_some(),
            };
            let past_cutoff = waited && wait_expired(&f).await;
            holder.rollback().await.unwrap();
            let result = match early {
                Some(value) => value,
                None => pending.as_mut().await,
            };
            drop(pending);
            f.auth.base.drain().await;
            f.assert_pool_restored().await;
            let hit = f.step_hit().await;
            let steps = f.steps().await;
            let effects = counts(&f).await;
            let terminal = unknown(&attempt);
            f.cleanup().await;
            assert!(
                waited && past_cutoff && hit && terminal,
                "phase={phase}, task={task}"
            );
            assert!(
                result.as_ref().is_err_and(expired),
                "expected fresh expiry, not timeout: {result:?}"
            );
            assert_eq!(steps.len(), usize::from(phase == "UPDATE"));
            assert!(steps.iter().all(|step| !step.2));
            assert_eq!(&effects[..5], &[0; 5]);
            // On phase B, validation can finish only after an immutable file was published.
            assert!(effects[5] <= i64::from(phase == "UPDATE" && !task));
        }
    }
}
