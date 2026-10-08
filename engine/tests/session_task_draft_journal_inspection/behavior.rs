use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_inspection_unknown_steps_do_not_resume_publication() {
    for task_step in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = f.journaled(ITEMS).await;
        let refs = attempt.allocated_artifacts().to_vec();
        if task_step {
            for _ in ITEMS {
                attempt.advance(&f.auth.learner_token).await.unwrap();
            }
        }
        f.resource_fault(task_step, "RETURN NULL;", false).await;
        let failed = attempt.advance(&f.auth.learner_token).await;
        let task = attempt.task_ref();
        let before = snapshot(&f).await;
        let inspected = inspect(&f, &f.auth.learner_token, task.task_id).await;
        let retry = attempt.advance(&f.auth.learner_token).await;
        let unchanged = before == snapshot(&f).await;
        let still_unknown = dispatch_fixture::unknown(&attempt);
        f.cleanup().await;
        assert!(failed.is_err() && unchanged && still_unknown);
        assert_eq!(retry, Err(dispatch_fixture::stopped()));
        let value = inspected.unwrap().unwrap();
        assert_eq!(value.4, if task_step { 2 } else { 0 });
        assert_eq!(
            value.5,
            Some(if task_step {
                SessionDraftPublicationStep::Task { reference: task }
            } else {
                SessionDraftPublicationStep::Artifact {
                    index: 0,
                    reference: refs[0].clone(),
                }
            })
        );
        assert!(!value.6);
        let checkpoint = SessionDraftPublicationCheckpoint::parse_json(&value.3).unwrap();
        assert_eq!(
            checkpoint.reported_state(),
            SessionDraftCheckpointState::Ready
        );
        assert_eq!(checkpoint.reported_confirmed_artifact_count(), 0);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_inspection_missing_foreign_and_account_mismatch_skip_steps() {
    let f = Fixture::ready().await;
    let attempt = f.journaled(ITEMS).await;
    let task = attempt.task_ref();
    query("INSERT INTO collaboration_draft_dispatch_steps (task_id, ordinal, nonce, committed) VALUES ($1, 0, $2, FALSE)")
        .bind(task.task_id.as_uuid()).bind(Uuid::nil()).execute(&f.journal_pool).await.unwrap();
    let before = snapshot(&f).await;
    let missing = inspect(&f, &f.auth.learner_token, id()).await;
    let foreign = inspect(&f, &f.auth.manager_token, task.task_id).await;
    let own_corruption = inspect(&f, &f.auth.learner_token, task.task_id).await;
    let intact = before == snapshot(&f).await;
    query("UPDATE collaboration_draft_intents SET account_id = $1")
        .bind(f.auth.target.account_id.as_uuid())
        .execute(&f.journal_pool)
        .await
        .unwrap();
    let before = snapshot(&f).await;
    let generation = inspect(&f, &f.auth.learner_token, task.task_id).await;
    let generation_intact = before == snapshot(&f).await;
    f.cleanup().await;
    assert_eq!(missing, Ok(None));
    assert_eq!(foreign, Ok(None));
    assert_eq!(generation, Ok(None));
    assert_eq!(
        own_corruption,
        Err(SessionDraftIntentError::Journal(
            DraftIntentJournalError::InvalidRecord
        ))
    );
    assert!(intact && generation_intact);
    // Exact account filtering is isolated by row corruption, not claimed account reincarnation.
}
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_inspection_reopens_fresh_session_and_reports_recorded_prefixes() {
    let f = Fixture::ready().await;
    let mut attempt = f.journaled(ITEMS).await;
    let task = attempt.task_ref();
    let checkpoint = attempt.checkpoint().unwrap().to_json().unwrap();
    let account = f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .unwrap()
        .account;
    let mut results = vec![inspect(&f, &f.auth.learner_token, task.task_id).await];
    for _ in 0..3 {
        attempt.advance(&f.auth.learner_token).await.unwrap();
        results.push(inspect(&f, &f.auth.learner_token, task.task_id).await);
    }
    drop(attempt);
    let mut empty = f.journaled(&[]).await;
    let empty_task = empty.task_ref();
    let empty_checkpoint = empty.checkpoint().unwrap().to_json().unwrap();
    let empty_before = inspect(&f, &f.auth.learner_token, empty_task.task_id).await;
    let empty_write = empty.advance(&f.auth.learner_token).await;
    let empty_after = inspect(&f, &f.auth.learner_token, empty_task.task_id).await;
    drop(empty);
    f.auth.source.logout(&f.auth.learner_token).await.unwrap();
    let fresh = f.auth.base.seed_session(&account).await.token;
    let source = f.auth.base.reopen().await;
    let pool = input_fixture::pool_for(&f.journal_schema).await;
    let journal = DraftIntentJournal::new(pool.clone(), scope(), f.installation).unwrap();
    let workspace =
        SessionDraftIntentWorkspace::new(&f.journal_schema, journal, f.workspace.clone()).unwrap();
    // Inspection must borrow the live source transaction, not connect via this closed pool.
    pool.close().await;
    let before = snapshot(&f).await;
    let loaded = source
        .inspect_session_draft_journal(&fresh, &workspace, task.task_id)
        .await
        .map(|value| value.as_ref().map(report));
    let unchanged = before == snapshot(&f).await;
    drop(workspace);
    drop(source);
    pool.close().await;
    let owner = f.auth.learner.principal.reference;
    f.cleanup().await;
    // First real behavior Red reaches this only after fixture cleanup.
    for (index, result) in results.into_iter().enumerate() {
        assert_eq!(
            result,
            Ok(Some((
                task,
                owner,
                account.account_id,
                checkpoint.clone(),
                index,
                None,
                index == 3
            )))
        );
    }
    assert_eq!(
        loaded,
        Ok(Some((
            task,
            owner,
            account.account_id,
            checkpoint,
            3,
            None,
            true
        )))
    );
    assert!(unchanged);
    assert_eq!(
        empty_write,
        Ok(SessionDraftPublicationProgress::TaskConfirmed)
    );
    assert_eq!(
        empty_before,
        Ok(Some((
            empty_task,
            owner,
            account.account_id,
            empty_checkpoint.clone(),
            0,
            None,
            false
        )))
    );
    assert_eq!(
        empty_after,
        Ok(Some((
            empty_task,
            owner,
            account.account_id,
            empty_checkpoint,
            1,
            None,
            true
        )))
    );
}
