use super::*;
pub use dispatch_fixture::Fixture;
pub use journal_fixture::ITEMS;
pub type Report = (
    SessionDraftPublicationStep,
    SessionDraftRecordedStepStatus,
    SessionDraftJournalObservedOutcome,
);
pub fn report(value: &SessionDraftJournalStepObservation) -> Report {
    (
        value.step().clone(),
        value.recorded_status(),
        value.result(),
    )
}
pub async fn observe(
    f: &Fixture,
    token: &str,
    task: TaskId,
    ordinal: usize,
) -> Result<Option<Report>, SessionDraftJournalObservationError> {
    f.auth
        .source
        .observe_session_draft_journal_step(token, &f.intent_workspace, task, ordinal)
        .await
        .map(|value| value.as_ref().map(report))
}
pub async fn complete(
    f: &Fixture,
    items: &[(&str, &str, &str)],
) -> SessionJournaledTaskDraftPublication {
    let mut attempt = f.journaled(items).await;
    for _ in 0..=items.len() {
        attempt.advance(&f.auth.learner_token).await.unwrap();
    }
    attempt
}
pub async fn uncertain(
    f: &Fixture,
    task: bool,
    items: &[(&str, &str, &str)],
) -> SessionJournaledTaskDraftPublication {
    let mut attempt = f.journaled(items).await;
    if task {
        for _ in items {
            attempt.advance(&f.auth.learner_token).await.unwrap();
        }
    }
    if task {
        f.sql_task("UPDATE collaboration_task_schema SET version = 999")
            .await;
    } else {
        f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
            .await;
    }
    let result = attempt.advance(&f.auth.learner_token).await;
    if task {
        f.sql_task("UPDATE collaboration_task_schema SET version = 3")
            .await;
    } else {
        f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 1")
            .await;
    }
    assert!(result.is_err());
    attempt
}
pub async fn publish(f: &Fixture, attempt: &SessionJournaledTaskDraftPublication, index: usize) {
    let reference = &attempt.allocated_artifacts()[index];
    f.auth
        .source
        .create_session_artifact(
            &f.auth.learner_token,
            &f.artifact_workspace,
            reference.artifact_id,
            reference.revision_id,
            plan(ITEMS).publication_draft(index).unwrap(),
        )
        .await
        .unwrap();
}
pub async fn create_task(
    f: &Fixture,
    attempt: &SessionJournaledTaskDraftPublication,
) -> TaskContentSnapshot {
    f.auth
        .source
        .create_session_content_task(
            &f.auth.learner_token,
            &f.workspace,
            attempt.task_ref().task_id,
            attempt.checkpoint().unwrap().manifest().clone(),
        )
        .await
        .unwrap()
}
pub async fn resource_lock(f: &Fixture, task: bool) -> dispatch_fixture::Tx<'_> {
    let mut tx = if task { &f.task_pool } else { &f.artifact_pool }
        .begin()
        .await
        .unwrap();
    let table = if task {
        "collaboration_task_schema"
    } else {
        "collaboration_artifact_schema"
    };
    query(&format!("SELECT singleton FROM {table} FOR UPDATE"))
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx
}
