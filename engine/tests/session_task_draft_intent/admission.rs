use super::*;

async fn closed_pool() -> PgPool {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    pool
}

#[tokio::test]
async fn journal_installation_requires_rfc_uuid_v4_without_connecting() {
    let pool = closed_pool().await;
    let mut reserved = *Uuid::new_v4().as_bytes();
    reserved[8] = 0;
    for installation in [
        Uuid::nil(),
        Uuid::parse_str("1223c6e9-823d-1cea-998a-a92f019b35fc").unwrap(),
        Uuid::from_bytes(reserved),
    ] {
        assert!(matches!(
            DraftIntentJournal::new(pool.clone(), scope(), installation),
            Err(DraftIntentJournalError::InvalidInput)
        ));
    }
    assert!(DraftIntentJournal::new(pool, scope(), Uuid::new_v4()).is_ok());
}

#[tokio::test]
async fn journal_workspace_rejects_unsafe_or_overlapping_names_and_wrong_scope() {
    let pool = closed_pool().await;
    let source = DurableAuthSource::new(pool.clone(), authority());
    let root = input_fixture::private_directory();
    let artifacts = source
        .open_artifact_workspace("intent_artifacts", scope(), &root)
        .unwrap();
    let content = SessionTaskContentWorkspace::new("intent_tasks", scope(), artifacts).unwrap();
    let journal = DraftIntentJournal::new(pool.clone(), scope(), Uuid::new_v4()).unwrap();
    let errors: Vec<_> = [
        "",
        "public",
        "pg_temp_1",
        "two,schemas",
        "../journal",
        "intent_tasks",
        "intent_artifacts",
    ]
    .into_iter()
    .map(|name| SessionDraftIntentWorkspace::new(name, journal.clone(), content.clone()).err())
    .collect();
    let foreign = DraftIntentJournal::new(
        pool,
        WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        Uuid::new_v4(),
    )
    .unwrap();
    let mismatch =
        SessionDraftIntentWorkspace::new("intent_journal", foreign, content.clone()).err();
    drop(content);
    fs::remove_dir(&root).unwrap();
    assert!(errors
        .into_iter()
        .all(|error| error == Some(SessionDraftIntentError::InvalidNamespace)));
    assert_eq!(
        mismatch,
        Some(SessionDraftIntentError::Journal(
            DraftIntentJournalError::ScopeMismatch
        ))
    );
}

#[tokio::test]
async fn journal_wrong_original_token_and_non_ready_attempt_fail_before_closed_sql() {
    let pool = closed_pool().await;
    let source = DurableAuthSource::new(pool.clone(), authority());
    let root = input_fixture::private_directory();
    let artifacts = source
        .open_artifact_workspace("intent_artifacts", scope(), &root)
        .unwrap();
    let content = SessionTaskContentWorkspace::new("intent_tasks", scope(), artifacts).unwrap();
    let journal = DraftIntentJournal::new(pool, scope(), Uuid::new_v4()).unwrap();
    let workspace =
        SessionDraftIntentWorkspace::new("intent_journal", journal, content.clone()).unwrap();
    let token = Uuid::new_v4().to_string();
    let mut attempt = source
        .prepare_task_draft_publication(&token, content, plan(&[]))
        .unwrap();
    let wrong = attempt
        .register_intent(&Uuid::new_v4().to_string(), &workspace)
        .await
        .err();
    let malformed = attempt
        .register_intent("not-a-session", &workspace)
        .await
        .err();
    let still_ready = attempt.state() == &SessionDraftPublicationState::Ready;
    let dispatched = attempt.advance(&token).await;
    let stopped = attempt.register_intent(&token, &workspace).await.err();
    drop(attempt);
    drop(workspace);
    fs::remove_dir(&root).unwrap();
    assert_eq!(wrong, Some(SessionDraftIntentError::SessionChanged));
    assert_eq!(malformed, Some(SessionDraftIntentError::SessionChanged));
    assert!(still_ready && dispatched.is_err());
    assert_eq!(stopped, Some(SessionDraftIntentError::InvalidAttempt));
}
