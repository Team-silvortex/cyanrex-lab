use super::*;
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_unknown_absent_or_present_never_resumes() {
    for task_step in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = uncertain(&f, task_step, ITEMS).await;
        let ordinal = if task_step { ITEMS.len() } else { 0 };
        let target = attempt.task_ref();
        let before = snapshot(&f).await;
        let absent = observe(&f, &f.auth.learner_token, target.task_id, ordinal).await;
        let absent_intact = before == snapshot(&f).await;
        if task_step {
            create_task(&f, &attempt).await;
        } else {
            publish(&f, &attempt, 0).await;
        }
        let before = snapshot(&f).await;
        let present = observe(&f, &f.auth.learner_token, target.task_id, ordinal).await;
        let retry = attempt.advance(&f.auth.learner_token).await;
        let intact = before == snapshot(&f).await && dispatch_fixture::unknown(&attempt);
        f.cleanup().await;
        let absent = absent.unwrap().unwrap();
        let present = present.unwrap().unwrap();
        assert_eq!(absent.0, present.0);
        assert_eq!(absent.1, SessionDraftRecordedStepStatus::Unknown);
        assert_eq!(present.1, SessionDraftRecordedStepStatus::Unknown);
        assert_eq!(absent.2, SessionDraftJournalObservedOutcome::NotVisible);
        assert_eq!(
            present.2,
            SessionDraftJournalObservedOutcome::MatchesIntentMetadata
        );
        assert_eq!(retry, Err(dispatch_fixture::stopped()));
        assert!(absent_intact && intact);
    }
}
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_filters_missing_foreign_and_unrecorded_targets() {
    let f = Fixture::ready().await;
    let attempt = f.journaled(ITEMS).await;
    let task = attempt.task_ref();
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
        .await;
    f.sql_task("UPDATE collaboration_task_schema SET version = 999")
        .await;
    let before = snapshot(&f).await;
    let unrecorded = observe(&f, &f.auth.learner_token, task.task_id, 0).await;
    let absent = observe(&f, &f.auth.learner_token, id(), 0).await;
    let foreign = observe(&f, &f.auth.manager_token, task.task_id, 0).await;
    let intact = before == snapshot(&f).await;
    query("INSERT INTO collaboration_draft_dispatch_steps VALUES ($1, 0, $2, FALSE)")
        .bind(task.task_id.as_uuid())
        .bind(Uuid::nil())
        .execute(&f.journal_pool)
        .await
        .unwrap();
    query("UPDATE collaboration_draft_intents SET account_id = $1")
        .bind(f.auth.target.account_id.as_uuid())
        .execute(&f.journal_pool)
        .await
        .unwrap();
    let before = snapshot(&f).await;
    let generation = observe(&f, &f.auth.learner_token, task.task_id, 0).await;
    let generation_intact = before == snapshot(&f).await;
    f.cleanup().await;
    assert_eq!(
        unrecorded,
        Err(SessionDraftJournalObservationError::NoRecordedStep)
    );
    assert_eq!(absent, Ok(None));
    assert_eq!(foreign, Ok(None));
    assert_eq!(generation, Ok(None));
    assert!(intact && generation_intact);
}
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_reports_later_task_edits_as_different() {
    let f = Fixture::ready().await;
    let attempt = complete(&f, ITEMS).await;
    let original = attempt.confirmed_task().unwrap().clone();
    let payload = original
        .manifest
        .payload()
        .iter()
        .map(|item| {
            TextPayloadBinding::new(item.artifact().clone(), "renamed.txt", "future.changed")
                .unwrap()
        })
        .collect();
    let edited = f
        .base
        .base
        .replace(
            &f.auth.learner_token,
            &original,
            TaskContentManifest::new("Changed title", payload).unwrap(),
        )
        .await
        .unwrap();
    let before = snapshot(&f).await;
    let changed = observe(
        &f,
        &f.auth.learner_token,
        attempt.task_ref().task_id,
        ITEMS.len(),
    )
    .await;
    let intact = before == snapshot(&f).await;
    f.base
        .base
        .transition(&f.auth.learner_token, &edited, TaskStatus::Ready)
        .await
        .unwrap();
    let before = snapshot(&f).await;
    let transitioned = observe(
        &f,
        &f.auth.learner_token,
        attempt.task_ref().task_id,
        ITEMS.len(),
    )
    .await;
    let status_intact = before == snapshot(&f).await;
    f.cleanup().await;
    assert_eq!(changed, transitioned);
    let value = changed.unwrap().unwrap();
    assert_eq!(value.1, SessionDraftRecordedStepStatus::Committed);
    assert_eq!(
        value.2,
        SessionDraftJournalObservedOutcome::DiffersFromIntentMetadata
    );
    assert!(intact && status_intact);
}
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_matches_committed_artifacts_and_tasks() {
    for items in [&[][..], ITEMS] {
        let f = Fixture::ready().await;
        let attempt = complete(&f, items).await;
        let task = attempt.task_ref();
        let account = f
            .auth
            .source
            .validate_session(&f.auth.learner_token)
            .await
            .unwrap()
            .unwrap()
            .account;
        let fresh = f.auth.base.seed_session(&account).await.token;
        let source = f.auth.base.reopen().await;
        let pool = input_fixture::pool_for(&f.journal_schema).await;
        let journal = DraftIntentJournal::new(pool.clone(), scope(), f.installation).unwrap();
        let workspace =
            SessionDraftIntentWorkspace::new(&f.journal_schema, journal, f.workspace.clone())
                .unwrap();
        pool.close().await;
        let before = snapshot(&f).await;
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        for ordinal in 0..=items.len() {
            actual.push(
                source
                    .observe_session_draft_journal_step(&fresh, &workspace, task.task_id, ordinal)
                    .await
                    .map(|value| value.as_ref().map(report)),
            );
            let step = if ordinal == items.len() {
                SessionDraftPublicationStep::Task { reference: task }
            } else {
                SessionDraftPublicationStep::Artifact {
                    index: ordinal,
                    reference: attempt.allocated_artifacts()[ordinal].clone(),
                }
            };
            expected.push(Ok(Some((
                step,
                SessionDraftRecordedStepStatus::Committed,
                SessionDraftJournalObservedOutcome::MatchesIntentMetadata,
            ))));
        }
        let intact = before == snapshot(&f).await;
        drop(workspace);
        drop(source);
        drop(attempt);
        f.cleanup().await;
        assert_eq!(actual, expected);
        assert!(intact);
    }
}
