use super::*;

pub type Inspected =
    Result<SessionDraftCheckpointObservation, SessionDraftCheckpointInspectionError>;

pub fn checkpoint(attempt: &SessionTaskDraftPublication) -> SessionDraftPublicationCheckpoint {
    let bytes = attempt.checkpoint().unwrap().to_json().unwrap();
    SessionDraftPublicationCheckpoint::parse_json(&bytes).unwrap()
}

pub fn changed(
    checkpoint: &SessionDraftPublicationCheckpoint,
    pointer: &str,
    value: Value,
) -> SessionDraftPublicationCheckpoint {
    let mut raw: Value = serde_json::from_slice(&checkpoint.to_json().unwrap()).unwrap();
    *raw.pointer_mut(pointer).unwrap() = value;
    SessionDraftPublicationCheckpoint::parse_json(&serde_json::to_vec(&raw).unwrap()).unwrap()
}

pub fn report(
    checkpoint: &SessionDraftPublicationCheckpoint,
    result: SessionDraftCheckpointObservedOutcome,
) -> Inspected {
    Ok(SessionDraftCheckpointObservation {
        step: checkpoint.reported_unconfirmed_step().unwrap(),
        result,
    })
}

pub async fn inspect(
    f: &Fixture,
    token: &str,
    checkpoint: &SessionDraftPublicationCheckpoint,
) -> Inspected {
    f.auth
        .source
        .inspect_task_draft_checkpoint(token, &f.workspace, checkpoint)
        .await
}

pub async fn visible(
    f: &Fixture,
    task: bool,
) -> (
    SessionTaskDraftPublication,
    SessionDraftPublicationCheckpoint,
) {
    let attempt = if task {
        let attempt = unknown_task(f, ITEMS).await;
        create_target(f, &attempt, ITEMS).await;
        attempt
    } else {
        let attempt = unknown_artifact(f, ITEMS, 0).await;
        publish_target(f, &attempt, ITEMS, 0).await;
        attempt
    };
    let checkpoint = checkpoint(&attempt);
    (attempt, checkpoint)
}

pub async fn fresh_same_owner(f: &Fixture) -> String {
    let account = f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .unwrap()
        .account;
    f.auth.base.seed_session(&account).await.token
}

pub async fn schema_lock(f: &Fixture, task: bool) -> Tx<'_> {
    let pool = if task { &f.task_pool } else { &f.artifact_pool };
    let table = if task {
        "collaboration_task_schema"
    } else {
        "collaboration_artifact_schema"
    };
    let mut holder = pool.begin().await.unwrap();
    query(&format!("SELECT singleton FROM {table} FOR UPDATE"))
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    holder
}

pub fn foreign_workspace(f: &Fixture, foreign_scope: WorkspaceRef) -> SessionTaskContentWorkspace {
    let artifacts = f
        .auth
        .source
        .open_artifact_workspace(&f.artifact_schema, foreign_scope, &f.directory)
        .unwrap();
    SessionTaskContentWorkspace::new(&f.task_schema, foreign_scope, artifacts).unwrap()
}
