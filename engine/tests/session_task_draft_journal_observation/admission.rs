use super::*;
async fn local() -> (PathBuf, DurableAuthSource, SessionDraftIntentWorkspace) {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool.clone(), authority());
    let root = input_fixture::private_directory();
    let artifacts = source
        .open_artifact_workspace("observe_artifacts", scope(), &root)
        .unwrap();
    let content = SessionTaskContentWorkspace::new("observe_tasks", scope(), artifacts).unwrap();
    let journal = DraftIntentJournal::new(pool, scope(), Uuid::new_v4()).unwrap();
    (
        root,
        source,
        SessionDraftIntentWorkspace::new("observe_journal", journal, content).unwrap(),
    )
}
#[tokio::test]
async fn journal_observation_bounds_ordinal_before_closed_storage() {
    let (root, source, workspace) = local().await;
    let token = Uuid::new_v4().to_string();
    let mut errors = Vec::new();
    for ordinal in [33, usize::MAX] {
        errors.push(
            source
                .observe_session_draft_journal_step(&token, &workspace, id(), ordinal)
                .await
                .err(),
        );
    }
    let accepted_bound = source
        .observe_session_draft_journal_step(&token, &workspace, id(), 32)
        .await
        .err();
    drop(workspace);
    fs::remove_dir(root).unwrap();
    assert_eq!(
        errors,
        vec![Some(SessionDraftJournalObservationError::InvalidOrdinal); 2]
    );
    assert_eq!(
        accepted_bound,
        Some(SessionDraftJournalObservationError::Intent(
            SessionDraftIntentError::Session(SessionCommandError::Authentication(
                DurableAuthError::StorageUnavailable
            ))
        ))
    );
}
#[tokio::test]
async fn journal_observation_rejects_noncanonical_tokens_before_closed_storage() {
    let (root, source, workspace) = local().await;
    let mut errors = Vec::new();
    for token in [
        "",
        "malformed",
        "00000000-0000-0000-0000-000000000000",
        "1223C6E9-823D-4CEA-998A-A92F019B35FC",
    ] {
        errors.push(
            source
                .observe_session_draft_journal_step(token, &workspace, id(), 0)
                .await
                .err(),
        );
    }
    drop(workspace);
    fs::remove_dir(root).unwrap();
    assert!(errors.into_iter().all(|error| error
        == Some(SessionDraftJournalObservationError::Intent(
            SessionDraftIntentError::Session(SessionCommandError::InvalidSession)
        ))));
}
