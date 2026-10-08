use super::*;
async fn closed() -> (PathBuf, DurableAuthSource, SessionDraftIntentWorkspace) {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool.clone(), authority());
    let root = input_fixture::private_directory();
    let artifacts = source
        .open_artifact_workspace("inspection_artifacts", scope(), &root)
        .unwrap();
    let content = SessionTaskContentWorkspace::new("inspection_tasks", scope(), artifacts).unwrap();
    let journal = DraftIntentJournal::new(pool, scope(), Uuid::new_v4()).unwrap();
    (
        root,
        source,
        SessionDraftIntentWorkspace::new("inspection_journal", journal, content).unwrap(),
    )
}
#[tokio::test]
async fn journal_inspection_rejects_noncanonical_session_tokens_before_closed_sql() {
    let (root, source, workspace) = closed().await;
    let mut results = Vec::new();
    for token in [
        "",
        "malformed",
        "00000000-0000-0000-0000-000000000000",
        "1223C6E9-823D-4CEA-998A-A92F019B35FC",
    ] {
        results.push(
            source
                .inspect_session_draft_journal(token, &workspace, id())
                .await
                .err(),
        );
    }
    drop(workspace);
    fs::remove_dir(root).unwrap();
    assert!(results.into_iter().all(|error| error
        == Some(SessionDraftIntentError::Session(
            SessionCommandError::InvalidSession
        ))));
}
#[tokio::test]
async fn journal_inspection_closed_storage_is_not_reported_as_absence_or_success() {
    let (root, source, workspace) = closed().await;
    let error = source
        .inspect_session_draft_journal(&Uuid::new_v4().to_string(), &workspace, id())
        .await
        .err();
    drop(workspace);
    fs::remove_dir(root).unwrap();
    assert_eq!(
        error,
        Some(SessionDraftIntentError::Session(
            SessionCommandError::Authentication(DurableAuthError::StorageUnavailable)
        )),
        "unavailable source storage must not become an absent intent"
    );
}
