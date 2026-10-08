use super::*;

async fn local() -> (
    PathBuf,
    DurableAuthSource,
    SessionTaskContentWorkspace,
    SessionDraftIntentWorkspace,
    String,
) {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool.clone(), authority());
    let root = input_fixture::private_directory();
    let artifacts = source
        .open_artifact_workspace("dispatch_artifacts", scope(), &root)
        .unwrap();
    let content = SessionTaskContentWorkspace::new("dispatch_tasks", scope(), artifacts).unwrap();
    let journal = DraftIntentJournal::new(pool, scope(), Uuid::new_v4()).unwrap();
    let workspace =
        SessionDraftIntentWorkspace::new("dispatch_journal", journal, content.clone()).unwrap();
    (root, source, content, workspace, Uuid::new_v4().to_string())
}

#[tokio::test]
async fn journaled_conversion_preserves_allocated_metadata_without_sql_or_dispatch() {
    let (root, source, content, workspace, token) = local().await;
    let original = source
        .prepare_task_draft_publication(&token, content, plan(ITEMS))
        .unwrap();
    let target = original.task_ref();
    let refs = original.allocated_artifacts().to_vec();
    let checkpoint = original.checkpoint().unwrap().to_json().unwrap();
    let mut attempt = original.into_journaled(workspace).unwrap();
    drop(attempt.advance(&token));
    let actual = attempt.checkpoint().unwrap().to_json().unwrap();
    let same = attempt.task_ref() == target
        && attempt.allocated_artifacts() == refs
        && attempt.confirmed_artifacts().is_empty()
        && attempt.confirmed_task().is_none()
        && attempt.state() == &SessionDraftPublicationState::Ready;
    drop(attempt);
    fs::remove_dir(root).unwrap();
    assert!(same);
    assert_eq!(actual, checkpoint);
}

#[tokio::test]
async fn journaled_conversion_rejects_mismatched_configuration_and_uncertain_attempts() {
    let (root, source, content, workspace, token) = local().await;
    let other = source
        .open_artifact_workspace("other_artifacts", scope(), &root)
        .unwrap();
    let other = SessionTaskContentWorkspace::new("other_tasks", scope(), other).unwrap();
    let wrong = source
        .prepare_task_draft_publication(&token, other, plan(&[]))
        .unwrap();
    let mismatch = wrong.into_journaled(workspace.clone()).err();
    let mut uncertain = source
        .prepare_task_draft_publication(&token, content, plan(&[]))
        .unwrap();
    let first = uncertain.advance(&token).await;
    let rejected = uncertain.into_journaled(workspace).err();
    fs::remove_dir(root).unwrap();
    assert_eq!(mismatch, Some(SessionDraftIntentError::InvalidAttempt));
    assert!(first.is_err());
    assert_eq!(rejected, Some(SessionDraftIntentError::InvalidAttempt));
}

#[tokio::test]
async fn journaled_wrong_token_is_local_but_dispatched_failure_is_terminal() {
    let (root, source, content, workspace, token) = local().await;
    let original = source
        .prepare_task_draft_publication(&token, content, plan(&[]))
        .unwrap();
    let mut attempt = original.into_journaled(workspace).unwrap();
    let wrong = attempt.advance(&Uuid::new_v4().to_string()).await;
    let malformed = attempt.advance("malformed").await;
    let ready = attempt.state() == &SessionDraftPublicationState::Ready;
    let failed = attempt.advance(&token).await;
    let uncertain = unknown(&attempt);
    let retry = attempt.advance(&token).await;
    drop(attempt);
    fs::remove_dir(root).unwrap();
    let changed = Err(SessionDraftDispatchError::Publication(
        SessionDraftPublicationError::SessionChanged,
    ));
    assert_eq!(wrong, changed);
    assert_eq!(malformed, changed);
    assert!(ready && failed.is_err() && uncertain);
    assert_eq!(retry, Err(stopped()));
}
