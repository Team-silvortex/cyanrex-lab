use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_empty_plan_works_through_independent_source_and_journal_handles() {
    let f = Fixture::ready().await;
    let source = f.auth.base.reopen().await;
    let pool = input_fixture::pool_for(&f.journal_schema).await;
    let journal = DraftIntentJournal::new(pool.clone(), scope(), f.installation).unwrap();
    let workspace =
        SessionDraftIntentWorkspace::new(&f.journal_schema, journal, f.workspace.clone()).unwrap();
    let original = source
        .prepare_task_draft_publication(&f.auth.learner_token, f.workspace.clone(), plan(&[]))
        .unwrap();
    original
        .register_intent(&f.auth.learner_token, &workspace)
        .await
        .unwrap();
    let mut attempt = original.into_journaled(workspace).unwrap();
    let result = attempt.advance(&f.auth.learner_token).await;
    let complete = attempt.state() == &SessionDraftPublicationState::Complete;
    let checkpoint = attempt.checkpoint().unwrap();
    let retry = attempt.advance(&f.auth.learner_token).await;
    let steps = f.steps().await;
    let effects = counts(&f).await;
    drop(attempt);
    drop(source);
    pool.close().await;
    f.cleanup().await;
    assert_eq!(result, Ok(SessionDraftPublicationProgress::TaskConfirmed));
    assert_eq!(retry, Err(stopped()));
    assert!(complete);
    assert_eq!(
        checkpoint.reported_state(),
        SessionDraftCheckpointState::Complete
    );
    assert_eq!(checkpoint.reported_confirmed_artifact_count(), 0);
    assert_eq!(steps.len(), 1);
    assert_eq!((steps[0].0, steps[0].2), (0, true));
    assert_eq!(effects, [0, 0, 0, 1, 1, 0]);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_missing_registration_and_wrong_token_never_publish() {
    let f = Fixture::ready().await;
    let mut missing = prepare(&f, ITEMS)
        .into_journaled(f.intent_workspace.clone())
        .unwrap();
    let absent = missing.advance(&f.auth.learner_token).await;
    let terminal = unknown(&missing);
    let retry = missing.advance(&f.auth.learner_token).await;
    let mut registered = f.journaled(ITEMS).await;
    let wrong = registered.advance(&f.auth.manager_token).await;
    let ready = registered.state() == &SessionDraftPublicationState::Ready;
    let effects = counts(&f).await;
    let steps = f.steps().await;
    f.cleanup().await;
    assert!(absent.is_err() && terminal && ready);
    assert_eq!(retry, Err(stopped()));
    assert_eq!(
        wrong,
        Err(SessionDraftDispatchError::Publication(
            SessionDraftPublicationError::SessionChanged
        ))
    );
    assert!(steps.is_empty());
    assert_eq!(effects, [0; 6]);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_schema_one_is_rejected_without_migration() {
    let f = intent_fixture::Fixture::ready().await;
    let original = prepare(&f, ITEMS);
    original
        .register_intent(&f.auth.learner_token, &f.intent_workspace)
        .await
        .unwrap();
    let mut attempt = original.into_journaled(f.intent_workspace.clone()).unwrap();
    let before = f.state().await;
    let result = attempt.advance(&f.auth.learner_token).await;
    let after = f.state().await;
    let no_table: bool =
        query("SELECT to_regclass('collaboration_draft_dispatch_steps') IS NULL AS missing")
            .fetch_one(&f.journal_pool)
            .await
            .unwrap()
            .get("missing");
    let version: i32 = query("SELECT version FROM collaboration_draft_intent_schema")
        .fetch_one(&f.journal_pool)
        .await
        .unwrap()
        .get("version");
    let effects = counts(&f).await;
    let terminal = unknown(&attempt);
    let reinstall = f.journal.install_empty_dispatch_namespace().await;
    f.cleanup().await;
    assert_eq!(
        result,
        Err(SessionDraftDispatchError::Intent(
            SessionDraftIntentError::Journal(DraftIntentJournalError::UnsupportedSchema)
        ))
    );
    assert_eq!(reinstall, Err(DraftIntentJournalError::NamespaceNotEmpty));
    assert_eq!(before, after);
    assert!(no_table && terminal);
    assert_eq!(version, 1);
    assert_eq!(effects, [0; 6]);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_dispatch_unicode_steps_commit_exact_resources_and_markers() {
    let f = Fixture::ready().await;
    let mut attempt = f.journaled(ITEMS).await;
    let target = attempt.task_ref();
    let refs = attempt.allocated_artifacts().to_vec();
    let mut results = Vec::new();
    let mut milestones = Vec::new();
    for _ in 0..3 {
        results.push(attempt.advance(&f.auth.learner_token).await);
        milestones.push((f.steps().await, counts(&f).await));
        if results.last().unwrap().is_err() {
            break;
        }
    }
    let state = attempt.state().clone();
    let confirmed = attempt.confirmed_artifacts().len();
    let confirmed_task = attempt.confirmed_task().map(|item| item.task.reference);
    let loaded = f.base.base.get(&f.auth.learner_token, target.task_id).await;
    let data = loaded
        .as_ref()
        .ok()
        .and_then(|value| value.as_ref())
        .map(|value| {
            (
                value.snapshot.manifest.input_refs(),
                value
                    .contents
                    .iter()
                    .map(|content| content.bytes.clone())
                    .collect::<Vec<_>>(),
            )
        });
    f.cleanup().await;
    // Cleanup precedes the real behavior Red against a no-dispatch stub.
    assert_eq!(
        results,
        vec![
            Ok(SessionDraftPublicationProgress::ArtifactConfirmed { index: 0 }),
            Ok(SessionDraftPublicationProgress::ArtifactConfirmed { index: 1 }),
            Ok(SessionDraftPublicationProgress::TaskConfirmed),
        ]
    );
    assert_eq!(state, SessionDraftPublicationState::Complete);
    assert_eq!(confirmed, 2);
    assert_eq!(confirmed_task, Some(target));
    assert_eq!(
        data,
        Some((
            refs,
            ITEMS
                .iter()
                .map(|item| item.2.as_bytes().to_vec())
                .collect()
        ))
    );
    for (index, (steps, effects)) in milestones.iter().enumerate() {
        assert_eq!(steps.len(), index + 1);
        assert!(steps
            .iter()
            .enumerate()
            .all(|(ordinal, step)| step.0 == ordinal as i32
                && step.2
                && step.1.get_version() == Some(uuid::Version::Random)));
        assert_eq!(
            *effects,
            if index == 0 {
                [1, 1, 1, 0, 0, 1]
            } else if index == 1 {
                [2, 2, 2, 0, 0, 2]
            } else {
                [2, 2, 2, 1, 1, 2]
            }
        );
    }
}
