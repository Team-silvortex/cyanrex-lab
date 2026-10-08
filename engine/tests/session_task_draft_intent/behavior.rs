use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_registration_reopens_ready_metadata_without_publication() {
    let f = Fixture::ready().await;
    let attempt = prepare(&f, ITEMS);
    let target = attempt.task_ref();
    let expected = attempt.checkpoint().unwrap().to_json().unwrap();
    let owner = f.auth.learner.principal.reference;
    let account = f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .unwrap()
        .account
        .account_id;
    let registered = attempt
        .register_intent(&f.auth.learner_token, &f.intent_workspace)
        .await;
    let reports_ready = registered.as_ref().is_ok_and(|item| {
        item.checkpoint().reported_state() == SessionDraftCheckpointState::Ready
            && item.checkpoint().reported_confirmed_artifact_count() == 0
            && item.checkpoint().reported_unconfirmed_step().is_none()
    });
    let registered = registered.as_ref().map(receipt).map_err(|error| *error);
    let ready = attempt.state() == &SessionDraftPublicationState::Ready
        && attempt.confirmed_artifacts().is_empty();
    drop(attempt);
    let pool = input_fixture::pool_for(&f.journal_schema).await;
    let reopened = DraftIntentJournal::new(pool.clone(), scope(), f.installation).unwrap();
    let workspace =
        SessionDraftIntentWorkspace::new(&f.journal_schema, reopened, f.workspace.clone()).unwrap();
    let source = DurableAuthSource::new(f.auth.base.pool.clone(), authority());
    let loaded = source
        .get_session_draft_intent(&f.auth.learner_token, &workspace, target.task_id)
        .await;
    let loaded = loaded
        .as_ref()
        .map(|item| item.as_ref().map(receipt))
        .map_err(|error| *error);
    let rows = f.count().await;
    let effects = counts(&f).await;
    drop(workspace);
    pool.close().await;
    f.cleanup().await;
    // All cleanup precedes the assertion that is deliberately Red against the registration stub.
    assert!(
        registered.is_ok(),
        "the current Session must persist its ready intent: {registered:?}"
    );
    let wanted = (target, owner, account, expected);
    assert_eq!(registered.unwrap(), wanted);
    assert_eq!(loaded.unwrap(), Some(wanted));
    assert!(ready && reports_ready);
    assert_eq!(rows, 1);
    assert_eq!(effects, [0; 6]);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_configured_resource_names_need_no_task_or_artifact_schema() {
    let f = Fixture::ready().await;
    let artifact_name = format!("future_artifacts_{}", Uuid::new_v4().simple());
    let task_name = format!("future_tasks_{}", Uuid::new_v4().simple());
    let artifacts = f
        .auth
        .source
        .open_artifact_workspace(&artifact_name, scope(), &f.directory)
        .unwrap();
    let content = SessionTaskContentWorkspace::new(&task_name, scope(), artifacts).unwrap();
    let workspace =
        SessionDraftIntentWorkspace::new(&f.journal_schema, f.journal.clone(), content.clone())
            .unwrap();
    let attempt = f
        .auth
        .source
        .prepare_task_draft_publication(&f.auth.learner_token, content, plan(&[]))
        .unwrap();
    let registered = attempt
        .register_intent(&f.auth.learner_token, &workspace)
        .await;
    let registered = registered.as_ref().map(receipt).map_err(|error| *error);
    let loaded = f
        .auth
        .source
        .get_session_draft_intent(
            &f.auth.learner_token,
            &workspace,
            attempt.task_ref().task_id,
        )
        .await;
    let loaded = loaded
        .as_ref()
        .map(|item| item.as_ref().map(receipt))
        .map_err(|error| *error);
    let wrong_routes = f
        .get(&f.auth.learner_token, attempt.task_ref().task_id)
        .await
        .err();
    let created_resources: i64 =
        query("SELECT count(*) AS n FROM pg_namespace WHERE nspname = ANY($1)")
            .bind(vec![artifact_name, task_name])
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("n");
    let effects = counts(&f).await;
    f.cleanup().await;
    assert_eq!(loaded.unwrap(), Some(registered.unwrap()));
    assert_eq!(
        wrong_routes,
        Some(SessionDraftIntentError::Journal(
            DraftIntentJournalError::InvalidRecord
        ))
    );
    assert_eq!(created_resources, 0);
    assert_eq!(effects, [0; 6]);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_duplicate_registration_across_independent_journal_handles_conflicts()
{
    let f = Fixture::ready().await;
    let attempt = prepare(&f, ITEMS);
    let pool = input_fixture::pool_for(&f.journal_schema).await;
    let other = DraftIntentJournal::new(pool.clone(), scope(), f.installation).unwrap();
    let workspace =
        SessionDraftIntentWorkspace::new(&f.journal_schema, other, f.workspace.clone()).unwrap();
    // Force a hostile deployment default on every source connection. The command must locally
    // request synchronous confirmation, without leaking that setting back to pooled connections.
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut connection = f.auth.base.pool.acquire().await.unwrap();
        query("SET synchronous_commit = 'off'")
            .execute(&mut *connection)
            .await
            .unwrap();
        held.push(connection);
    }
    drop(held);
    f.fault("IF current_setting('synchronous_commit') <> 'on' THEN RAISE EXCEPTION 'intent must enter with synchronous commit'; END IF;
        PERFORM set_config('synchronous_commit', 'off', true);", false).await;
    f.sql("CREATE SEQUENCE intent_sync_commit_calls").await;
    f.sql(&format!("CREATE FUNCTION check_intent_sync_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.intent_sync_commit_calls');
        IF current_setting('synchronous_commit') <> 'on' THEN RAISE EXCEPTION 'intent must restore synchronous commit before confirming'; END IF;
        RETURN NEW; END $$", f.journal_schema)).await;
    f.sql("CREATE CONSTRAINT TRIGGER check_intent_sync_commit AFTER INSERT ON collaboration_draft_intents DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION check_intent_sync_commit()").await;
    let (a, b) = tokio::join!(
        attempt.register_intent(&f.auth.learner_token, &f.intent_workspace),
        attempt.register_intent(&f.auth.learner_token, &workspace),
    );
    let successes = usize::from(a.is_ok()) + usize::from(b.is_ok());
    let errors = [a.err(), b.err()];
    let before = f.state().await;
    let duplicate = attempt
        .register_intent(&f.auth.learner_token, &workspace)
        .await
        .err();
    let unchanged = before == f.state().await;
    let hit = f.hit().await;
    let commit_hit: bool = query("SELECT is_called FROM intent_sync_commit_calls")
        .fetch_one(&f.journal_pool)
        .await
        .unwrap()
        .get("is_called");
    let rows = f.count().await;
    let mut restored = true;
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut connection = f.auth.base.pool.acquire().await.unwrap();
        let setting: String = query("SHOW synchronous_commit")
            .fetch_one(&mut *connection)
            .await
            .unwrap()
            .get(0);
        restored &= setting == "off";
        held.push(connection);
    }
    drop(held);
    drop(workspace);
    pool.close().await;
    let effects = counts(&f).await;
    f.cleanup().await;
    let conflict = SessionDraftIntentError::Journal(DraftIntentJournalError::Conflict);
    assert_eq!(successes, 1);
    assert_eq!(
        errors
            .iter()
            .filter(|error| **error == Some(conflict))
            .count(),
        1
    );
    assert_eq!(duplicate, Some(conflict));
    assert!(unchanged && hit && commit_hit && restored);
    assert_eq!(rows, 1);
    assert_eq!(effects, [0; 6]);
}
