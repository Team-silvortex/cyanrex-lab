use super::*;

pub async fn pair() -> (Fixture, TaskSnapshot, ArtifactRevision, ArtifactRevision) {
    let f = Fixture::ready().await;
    let old = f
        .create_artifact(&f.auth.learner_token, b"Original input")
        .await;
    let new = f
        .auth
        .source
        .revise_session_artifact(
            &f.auth.learner_token,
            &f.artifact_workspace,
            &old.reference,
            id(),
            draft(b"Revised input"),
        )
        .await
        .unwrap();
    let task = f
        .create(&f.auth.learner_token, vec![old.reference.clone()])
        .await;
    (f, task, old, new)
}

pub async fn replace(
    f: &Fixture,
    token: &str,
    task: &TaskSnapshot,
    refs: Vec<ArtifactRef>,
) -> Result<TaskSnapshot, SessionTaskError> {
    f.auth
        .source
        .replace_session_input_task_inputs(
            token,
            &f.workspace,
            task.reference.task_id,
            task.revision,
            refs,
        )
        .await
}

pub fn catalog_workspace(
    f: &Fixture,
    definitions: Vec<TaskDefinition>,
) -> SessionCatalogTaskWorkspace {
    SessionCatalogTaskWorkspace::new(
        f.task_workspace.clone(),
        f.artifact_workspace.clone(),
        &catalog_fixture::catalog(definitions),
    )
    .unwrap()
}

pub async fn catalog_task(
    f: &Fixture,
    workspace: &SessionCatalogTaskWorkspace,
    refs: Vec<ArtifactRef>,
) -> TaskSnapshot {
    f.auth
        .source
        .create_session_catalog_task(
            &f.auth.learner_token,
            workspace,
            id(),
            &catalog_fixture::reference(),
            "Private catalog draft",
            refs,
        )
        .await
        .unwrap()
}

pub async fn catalog_replace(
    f: &Fixture,
    workspace: &SessionCatalogTaskWorkspace,
    task: &TaskSnapshot,
    refs: Vec<ArtifactRef>,
) -> Result<TaskSnapshot, SessionTaskError> {
    f.auth
        .source
        .replace_session_catalog_task_inputs(
            &f.auth.learner_token,
            workspace,
            task.reference.task_id,
            task.revision,
            refs,
        )
        .await
}

pub async fn unchanged(f: &Fixture, task: &TaskSnapshot, events: i64) {
    assert_eq!(
        f.tasks
            .get(task.reference, task.owner)
            .await
            .unwrap()
            .as_ref(),
        Some(task)
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, events);
}

pub async fn published(f: &Fixture, old: &ArtifactRevision, new: &ArtifactRevision) {
    for (revision, bytes) in [
        (old, b"Original input".as_slice()),
        (new, b"Revised input".as_slice()),
    ] {
        let content = f
            .artifacts
            .read(&revision.reference, revision.owner)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&content.revision, revision);
        assert_eq!(content.bytes, bytes);
    }
    assert_eq!(f.count_artifact("collaboration_artifacts").await, 1);
    assert_eq!(
        f.count_artifact("collaboration_artifact_revisions").await,
        2
    );
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 2);
    assert_eq!(f.files(), 2);
}

pub async fn event_types(f: &Fixture, task: &TaskSnapshot) -> Vec<String> {
    query("SELECT event_type FROM collaboration_task_outbox WHERE task_id = $1 ORDER BY revision")
        .bind(task.reference.task_id.as_uuid())
        .fetch_all(&f.task_pool)
        .await
        .unwrap()
        .iter()
        .map(|row| row.get("event_type"))
        .collect()
}

pub async fn fault_trigger(f: &Fixture, body: &str) {
    f.sql_task("CREATE SEQUENCE replacement_fault_calls").await;
    f.sql_task(&format!(
        "CREATE FUNCTION replacement_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.replacement_fault_calls'); {body} RETURN NEW; END $$",
        f.task_schema
    ))
    .await;
    f.sql_task(
        "CREATE TRIGGER replacement_fault BEFORE INSERT ON collaboration_task_outbox
        FOR EACH ROW EXECUTE FUNCTION replacement_fault()",
    )
    .await;
}

pub async fn fault_reached(f: &Fixture) {
    let row = query("SELECT last_value, is_called FROM replacement_fault_calls")
        .fetch_one(&f.task_pool)
        .await
        .unwrap();
    assert!(row.get::<bool, _>("is_called"));
    assert_eq!(row.get::<i64, _>("last_value"), 1);
}
