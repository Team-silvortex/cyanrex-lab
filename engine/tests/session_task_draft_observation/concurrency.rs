use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_pending_creator_visibility_does_not_prove_final_outcome() {
    for commit in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = unknown_artifact(&f, ITEMS, 0).await;
        let state = attempt.state().clone();
        let reference = attempt.allocated_artifacts()[0].clone();
        let source = f.auth.source.clone();
        let workspace = f.artifact_workspace.clone();
        let token = f.auth.learner_token.clone();
        let draft = plan(ITEMS).publication_draft(0).unwrap();
        let mut holder = pause_artifact_event(&f).await;
        let blocker = backend(&mut holder).await;
        let creator = tokio::spawn(async move {
            source
                .create_session_artifact(
                    &token,
                    &workspace,
                    reference.artifact_id,
                    reference.revision_id,
                    draft,
                )
                .await
        });
        let creator_pid = blocked_by(&f, blocker).await;
        let mut pending = Box::pin(attempt.observe_unconfirmed(&f.auth.learner_token));
        let mut early = None;
        let read_waited = if let Some(pid) = creator_pid {
            tokio::select! {
                result = &mut pending => { early = Some(result); false },
                waiter = blocked_by(&f, pid) => waiter.is_some(),
            }
        } else {
            false
        };
        // READ COMMITTED may not see an uncommitted INSERT at all. A legitimate early None is
        // precisely why NotVisible cannot be interpreted as a terminal publication outcome.
        if !commit {
            creator.abort();
        }
        holder.rollback().await.unwrap();
        let created = creator.await;
        let first = match early {
            Some(result) => result,
            None => pending.as_mut().await,
        };
        drop(pending);
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        let before = snapshot(&f).await;
        let final_read = attempt.observe_unconfirmed(&f.auth.learner_token).await;
        let after = snapshot(&f).await;
        let absent = report(&attempt, SessionDraftObservedOutcome::NotVisible);
        let matching = report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate);
        let final_state = attempt.state().clone();
        let empty = attempt.confirmed_artifacts().is_empty() && attempt.confirmed_task().is_none();
        let stopped = attempt.advance(&f.auth.learner_token).await;
        let remaining = counts(&f).await;
        f.cleanup().await;
        assert!(
            creator_pid.is_some(),
            "creator must first reach the exact outbox blocker"
        );
        if commit {
            assert!(created.unwrap().is_ok());
            assert!(first == absent || (read_waited && first == matching));
            assert_eq!(final_read, matching);
            assert_eq!(remaining, [1, 1, 1, 0, 0, 1]);
        } else {
            assert!(created.is_err());
            assert_eq!(first, absent);
            assert_eq!(final_read, absent);
            assert_eq!(remaining, [0, 0, 0, 0, 0, 1]);
        }
        assert_eq!(before, after);
        assert_eq!(state, final_state);
        assert!(empty);
        assert_eq!(stopped, Err(SessionDraftPublicationError::Stopped));
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_cancelled_read_preserves_progress_sql_files_and_pool() {
    for task in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = if task {
            let attempt = unknown_task(&f, ITEMS).await;
            create_target(&f, &attempt, ITEMS).await;
            attempt
        } else {
            let attempt = unknown_artifact(&f, ITEMS, 0).await;
            publish_target(&f, &attempt, ITEMS, 0).await;
            attempt
        };
        let state = attempt.state().clone();
        let receipts = attempt.confirmed_artifacts().to_vec();
        let before = snapshot(&f).await;
        let pool = if task { &f.task_pool } else { &f.artifact_pool };
        let table = if task {
            "collaboration_task_schema"
        } else {
            "collaboration_artifact_schema"
        };
        let mut holder = pool.begin().await.unwrap();
        query(&format!("SELECT singleton FROM {table} FOR UPDATE"))
            .fetch_one(&mut *holder)
            .await
            .unwrap();
        let blocker = backend(&mut holder).await;
        let mut pending = Box::pin(attempt.observe_unconfirmed(&f.auth.learner_token));
        let waited = tokio::select! {
            _ = &mut pending => false,
            waiter = blocked_by(&f, blocker) => waiter.is_some(),
        };
        drop(pending);
        holder.rollback().await.unwrap();
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        let after_cancel = snapshot(&f).await;
        let later = attempt.observe_unconfirmed(&f.auth.learner_token).await;
        let after_later = snapshot(&f).await;
        let expected = report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate);
        let final_state = attempt.state().clone();
        let final_receipts = attempt.confirmed_artifacts().to_vec();
        let stopped = attempt.advance(&f.auth.learner_token).await;
        f.cleanup().await;
        assert!(
            waited,
            "cancel only after this read reaches the exact resource metadata lock"
        );
        assert_eq!(before, after_cancel);
        assert_eq!(before, after_later);
        assert_eq!(later, expected);
        assert_eq!(state, final_state);
        assert_eq!(receipts, final_receipts);
        assert_eq!(stopped, Err(SessionDraftPublicationError::Stopped));
    }
}
