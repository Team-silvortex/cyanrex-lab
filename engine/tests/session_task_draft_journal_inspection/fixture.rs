use super::*;
pub use dispatch_fixture::{Fixture, ITEMS};
pub type Report = (
    TaskRef,
    PrincipalRef,
    LegacyAccountId,
    Vec<u8>,
    usize,
    Option<SessionDraftPublicationStep>,
    bool,
);
pub fn report(value: &SessionDraftJournalInspection) -> Report {
    (
        value.intent().task_ref(),
        value.intent().owner(),
        value.intent().account_id(),
        value.intent().checkpoint().to_json().unwrap(),
        value.recorded_committed_step_count(),
        value.unknown_step().cloned(),
        value.all_steps_recorded_committed(),
    )
}
pub async fn inspect(
    f: &Fixture,
    token: &str,
    task: TaskId,
) -> Result<Option<Report>, SessionDraftIntentError> {
    f.auth
        .source
        .inspect_session_draft_journal(token, &f.intent_workspace, task)
        .await
        .map(|value| value.as_ref().map(report))
}
pub async fn snapshot(f: &Fixture) -> (Vec<String>, Vec<(String, Vec<u8>)>) {
    let mut rows = vec![f.state().await, f.metadata().await, f.source_state().await];
    for (pool, tables) in [
        (
            &f.journal_pool,
            &["collaboration_draft_dispatch_steps"] as &[_],
        ),
        (
            &f.artifact_pool,
            &[
                "collaboration_artifacts",
                "collaboration_artifact_revisions",
                "collaboration_artifact_outbox",
            ] as &[_],
        ),
        (
            &f.task_pool,
            &["collaboration_tasks", "collaboration_task_outbox"] as &[_],
        ),
    ] {
        for table in tables {
            rows.push(query(&format!("SELECT md5(COALESCE(string_agg(row_to_json(t)::text, '' ORDER BY row_to_json(t)::text), '')) AS hash FROM {table} t"))
                .fetch_one(pool).await.unwrap().get("hash"));
        }
    }
    let mut files: Vec<_> = fs::read_dir(&f.directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_str().unwrap().to_owned(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect();
    files.sort();
    (rows, files)
}
