use super::*;

pub fn definition(version: u64) -> TaskDefinition {
    let pin = |name: &str| VersionedName {
        name: name.parse().unwrap(),
        version: 1.try_into().unwrap(),
    };
    TaskDefinition {
        schema_version: CoreSchemaVersion,
        reference: TaskDefinitionRef {
            package: pin("sample.document"),
            name: "sample.document.prepare".parse().unwrap(),
            version: version.try_into().unwrap(),
        },
        title: "Prepare a document".into(),
        summary: "Private text preparation without an evaluator or Run".into(),
        evidence_schema: Some(pin("sample.document.text")),
        assessment_policy: Some(pin("sample.document.editorial")),
    }
}

pub fn definitions(values: Vec<TaskDefinition>) -> TaskDefinitionCatalog {
    TaskDefinitionCatalog::new(definition(1).reference.package, values).unwrap()
}

pub fn workspace(
    f: &Fixture,
    values: &TaskDefinitionCatalog,
) -> SessionCatalogTaskContentWorkspace {
    SessionCatalogTaskContentWorkspace::from_definitions(
        &f.task_schema,
        scope(),
        f.artifact_workspace.clone(),
        values,
    )
    .unwrap()
}

pub async fn create(
    f: &Fixture,
    workspace: &SessionCatalogTaskContentWorkspace,
    value: TaskContentManifest,
) -> TaskContentSnapshot {
    f.auth
        .source
        .create_session_catalog_content_task(
            &f.auth.learner_token,
            workspace,
            id(),
            &definition(1).reference,
            value,
        )
        .await
        .unwrap()
}

pub async fn get(
    f: &Fixture,
    workspace: &SessionCatalogTaskContentWorkspace,
    task: &TaskContentSnapshot,
) -> Result<Option<SessionTaskContent>, SessionTaskError> {
    f.auth
        .source
        .get_session_catalog_content_task(
            &f.auth.learner_token,
            workspace,
            task.task.reference.task_id,
        )
        .await
}

pub async fn replace(
    f: &Fixture,
    workspace: &SessionCatalogTaskContentWorkspace,
    task: &TaskContentSnapshot,
    value: TaskContentManifest,
) -> Result<TaskContentSnapshot, SessionTaskError> {
    f.auth
        .source
        .replace_session_catalog_content_task(
            &f.auth.learner_token,
            workspace,
            task.task.reference.task_id,
            task.task.revision,
            value,
        )
        .await
}

pub async fn transition(
    f: &Fixture,
    workspace: &SessionCatalogTaskContentWorkspace,
    task: &TaskContentSnapshot,
    status: TaskStatus,
) -> Result<TaskContentSnapshot, SessionTaskError> {
    f.auth
        .source
        .transition_session_catalog_content_task(
            &f.auth.learner_token,
            workspace,
            task.task.reference.task_id,
            task.task.revision,
            status,
        )
        .await
}

pub async fn pair() -> (
    Fixture,
    SessionCatalogTaskContentWorkspace,
    TaskContentSnapshot,
    ArtifactRevision,
    ArtifactRevision,
) {
    let f = Fixture::ready().await;
    let workspace = workspace(&f, &definitions(vec![definition(1)]));
    let old = f
        .create_artifact(&f.auth.learner_token, b"Original input")
        .await;
    let new = f
        .create_artifact(&f.auth.learner_token, b"Replacement input")
        .await;
    let task = create(
        &f,
        &workspace,
        manifest("Catalog content", vec![old.reference.clone()]),
    )
    .await;
    (f, workspace, task, old, new)
}

/// Capture values, not just counts: rejected operations must preserve heads and event histories.
pub async fn business_rows(f: &Fixture) -> Vec<Vec<String>> {
    let mut snapshots = Vec::new();
    for (pool, table) in [
        (&f.task_pool, "collaboration_task_schema"),
        (&f.task_pool, "collaboration_tasks"),
        (&f.task_pool, "collaboration_task_outbox"),
        (&f.artifact_pool, "collaboration_artifact_schema"),
        (&f.artifact_pool, "collaboration_artifacts"),
        (&f.artifact_pool, "collaboration_artifact_revisions"),
        (&f.artifact_pool, "collaboration_artifact_outbox"),
    ] {
        let mut rows: Vec<String> = query(&format!(
            "SELECT row_to_json(t)::text AS snapshot FROM {table} t"
        ))
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get("snapshot"))
        .collect();
        rows.sort();
        snapshots.push(rows);
    }
    snapshots
}

#[tokio::test]
async fn session_catalog_content_configuration_is_provider_free_frozen_and_scope_checked() {
    fn shareable<T: Clone + Send + Sync>() {}
    shareable::<SessionCatalogTaskContentWorkspace>();
    let source = input_fixture::unavailable_source().await;
    let directory = input_fixture::private_directory();
    let artifacts = source
        .open_artifact_workspace("private_artifacts", scope(), &directory)
        .unwrap();
    let values = definitions(vec![definition(1), definition(2)]);
    let copied = SessionCatalogTaskContentWorkspace::from_definitions(
        "private_content",
        scope(),
        artifacts.clone(),
        &values,
    )
    .unwrap();
    drop(values);
    drop(copied.clone());
    for name in [
        "",
        "public",
        "pg_catalog",
        "a.b",
        "has space",
        "private_artifacts",
    ] {
        assert!(
            matches!(
                SessionCatalogTaskContentWorkspace::from_definitions(
                    name,
                    scope(),
                    artifacts.clone(),
                    &definitions(vec![definition(1)]),
                ),
                Err(SessionTaskError::InvalidNamespace)
            ),
            "{name}"
        );
    }
    for other in [
        WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        WorkspaceRef {
            authority_id: id(),
            ..scope()
        },
    ] {
        assert!(matches!(
            SessionCatalogTaskContentWorkspace::from_definitions(
                "private_content",
                other,
                artifacts.clone(),
                &definitions(vec![definition(1)]),
            ),
            Err(SessionTaskError::Task(TaskStoreError::ScopeMismatch))
        ));
    }
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop((copied, artifacts));
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn session_catalog_content_closed_source_never_falls_back_and_unknown_create_needs_no_database(
) {
    let source = input_fixture::unavailable_source().await;
    let directory = input_fixture::private_directory();
    let artifacts = source
        .open_artifact_workspace("private_artifacts", scope(), &directory)
        .unwrap();
    let workspace = SessionCatalogTaskContentWorkspace::from_definitions(
        "private_content",
        scope(),
        artifacts,
        &definitions(vec![definition(1)]),
    )
    .unwrap();
    for (token, expected) in [
        (
            "invalid".to_string(),
            SessionTaskError::Session(SessionCommandError::InvalidSession),
        ),
        (
            Uuid::new_v4().to_string(),
            SessionTaskError::Session(SessionCommandError::Authentication(
                DurableAuthError::StorageUnavailable,
            )),
        ),
    ] {
        let task_id = id();
        let value = manifest("Empty task", vec![]);
        assert_eq!(
            source
                .create_session_catalog_content_task(
                    &token,
                    &workspace,
                    task_id,
                    &definition(1).reference,
                    value.clone()
                )
                .await,
            Err(expected)
        );
        assert_eq!(
            source
                .get_session_catalog_content_task(&token, &workspace, task_id)
                .await,
            Err(expected)
        );
        assert_eq!(
            source
                .replace_session_catalog_content_task(
                    &token,
                    &workspace,
                    task_id,
                    1.try_into().unwrap(),
                    value
                )
                .await,
            Err(expected)
        );
        assert_eq!(
            source
                .transition_session_catalog_content_task(
                    &token,
                    &workspace,
                    task_id,
                    1.try_into().unwrap(),
                    TaskStatus::Ready
                )
                .await,
            Err(expected)
        );
    }
    assert_eq!(
        source
            .create_session_catalog_content_task(
                &Uuid::new_v4().to_string(),
                &workspace,
                id(),
                &definition(2).reference,
                manifest("No version fallback", vec![]),
            )
            .await,
        Err(SessionTaskError::Task(TaskStoreError::UnknownDefinition))
    );
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop(workspace);
    fs::remove_dir(directory).unwrap();
}
