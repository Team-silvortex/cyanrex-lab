//! One explicitly configured workspace admits several exact metadata packages, not providers.
use super::*;
use cyanrex_engine::services::task_catalog::TaskDefinitionCatalog;

fn value(package: &str, package_version: u64, task_version: u64) -> TaskDefinition {
    let pin = |name: String| VersionedName {
        name: name.parse().unwrap(),
        version: package_version.try_into().unwrap(),
    };
    TaskDefinition {
        schema_version: CoreSchemaVersion,
        reference: TaskDefinitionRef {
            package: pin(package.into()),
            name: format!("{package}.prepare").parse().unwrap(),
            version: task_version.try_into().unwrap(),
        },
        title: format!("Definition {package} {package_version}/{task_version}"),
        summary: "Frozen multi-domain metadata, not executable rules".into(),
        evidence_schema: Some(pin(format!("{package}.text"))),
        assessment_policy: Some(pin(format!("{package}.review"))),
    }
}

fn combined(values: &[TaskDefinition]) -> TaskDefinitionCatalog {
    let catalogs: Vec<_> = values
        .iter()
        .map(|definition| {
            TaskDefinitionCatalog::new(
                definition.reference.package.clone(),
                vec![definition.clone()],
            )
            .unwrap()
        })
        .collect();
    // Same package/version disjoint definitions are deliberately supplied as separate catalogs.
    TaskDefinitionCatalog::compose(&catalogs).unwrap()
}

fn configured(f: &Fixture, values: &[TaskDefinition]) -> SessionCatalogTaskWorkspace {
    let metadata = combined(values);
    let workspace = SessionCatalogTaskWorkspace::from_definitions(
        f.task_workspace.clone(),
        f.artifact_workspace.clone(),
        &metadata,
    )
    .unwrap();
    drop(metadata);
    workspace
}

async fn business_rows(f: &Fixture) -> Vec<Vec<String>> {
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

fn file_values(f: &Fixture) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<_> = fs::read_dir(&f.directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().into_string().unwrap(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect();
    files.sort();
    files
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_composed_packages_keep_exact_versions_and_isolated_admission(
) {
    let f = Fixture::ready().await;
    let values = vec![
        value("sample.document", 1, 1),
        value("sample.document", 1, 2),
        value("sample.document", 2, 1),
        value("sample.editorial", 1, 1),
    ];
    let workspace = configured(&f, &values);
    let token = &f.auth.learner_token;
    let original = f.create_artifact(token, b"\tOriginal document\r\n").await;
    let next = f.create_artifact(token, &[0, 255, 1]).await;
    let inputs = vec![original.reference.clone(), next.reference.clone()];
    let mut tasks = Vec::new();
    for definition in &values {
        let task = f
            .auth
            .source
            .create_session_catalog_task(
                token,
                &workspace,
                id(),
                &definition.reference,
                "Instance title independent of the definition",
                inputs.clone(),
            )
            .await
            .unwrap();
        assert_eq!(task.definition.as_ref(), Some(definition));
        assert_eq!(task.owner, f.auth.learner.principal.reference);
        let read = f
            .auth
            .source
            .get_session_catalog_task(token, &workspace, task.reference.task_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read.task, task);
        assert_eq!(read.inputs[0].bytes, b"\tOriginal document\r\n");
        assert_eq!(read.inputs[1].bytes, [0, 255, 1]);
        tasks.push(task);
    }
    let changed = f
        .auth
        .source
        .replace_session_catalog_task_inputs(
            token,
            &workspace,
            tasks[1].reference.task_id,
            tasks[1].revision,
            vec![next.reference.clone(), original.reference.clone()],
        )
        .await
        .unwrap();
    let ready = f
        .auth
        .source
        .transition_session_catalog_task(
            token,
            &workspace,
            changed.reference.task_id,
            changed.revision,
            TaskStatus::Ready,
        )
        .await
        .unwrap();
    assert_eq!(ready.definition.as_ref(), Some(&values[1]));
    assert_eq!(ready.created_at, tasks[1].created_at);
    assert_eq!(ready.owner, tasks[1].owner);
    assert_eq!(u64::from(ready.revision), 3);
    let read = f
        .auth
        .source
        .get_session_catalog_task(token, &workspace, ready.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.task, ready);
    assert_eq!(read.inputs[0].bytes, [0, 255, 1]);
    assert_eq!(read.inputs[1].bytes, b"\tOriginal document\r\n");

    let target = &tasks[2];
    let mut drifted = values.clone();
    drifted[2].summary = "Drift under the same exact definition pin".into();
    assert_ne!(
        drifted[2], values[2],
        "the fault must change saved metadata"
    );
    let rejected = [
        (
            vec![values[3].clone()],
            SessionTaskError::Task(TaskStoreError::UnknownDefinition),
        ),
        (
            vec![values[0].clone(), values[1].clone(), values[3].clone()],
            SessionTaskError::Task(TaskStoreError::UnknownDefinition),
        ),
        (drifted, SessionTaskError::DefinitionMismatch),
    ];
    let before = business_rows(&f).await;
    let files = file_values(&f);
    for (definitions, expected) in rejected {
        let selected = configured(&f, &definitions);
        for result in [
            f.auth
                .source
                .get_session_catalog_task(token, &selected, target.reference.task_id)
                .await
                .map(|_| ()),
            f.auth
                .source
                .replace_session_catalog_task_inputs(
                    token,
                    &selected,
                    target.reference.task_id,
                    target.revision,
                    vec![],
                )
                .await
                .map(|_| ()),
            f.auth
                .source
                .transition_session_catalog_task(
                    token,
                    &selected,
                    target.reference.task_id,
                    target.revision,
                    TaskStatus::Ready,
                )
                .await
                .map(|_| ()),
        ] {
            assert_eq!(result, Err(expected));
        }
        // Rejecting one package never hides an unchanged admitted package in that same workspace.
        assert_eq!(
            f.auth
                .source
                .get_session_catalog_task(token, &selected, tasks[3].reference.task_id)
                .await
                .unwrap()
                .unwrap()
                .task,
            tasks[3]
        );
        assert_eq!(business_rows(&f).await, before);
        assert_eq!(file_values(&f), files);
    }
    for mut unknown in [values[0].reference.clone(), values[2].reference.clone()] {
        unknown.version = 3.try_into().unwrap();
        assert_eq!(
            f.auth
                .source
                .create_session_catalog_task(
                    token,
                    &workspace,
                    id(),
                    &unknown,
                    "No floating version",
                    vec![]
                )
                .await,
            Err(SessionTaskError::Task(TaskStoreError::UnknownDefinition))
        );
    }
    for task in &tasks {
        assert_eq!(
            f.auth
                .source
                .get_session_catalog_task(&f.auth.manager_token, &workspace, task.reference.task_id)
                .await
                .unwrap(),
            None
        );
    }
    for result in [
        f.auth
            .source
            .replace_session_catalog_task_inputs(
                &f.auth.manager_token,
                &workspace,
                target.reference.task_id,
                target.revision,
                vec![],
            )
            .await
            .map(|_| ()),
        f.auth
            .source
            .transition_session_catalog_task(
                &f.auth.manager_token,
                &workspace,
                target.reference.task_id,
                target.revision,
                TaskStatus::Ready,
            )
            .await
            .map(|_| ()),
    ] {
        assert_eq!(
            result,
            Err(SessionTaskError::Task(TaskStoreError::NotFound))
        );
    }
    assert_eq!(business_rows(&f).await, before);
    assert_eq!(file_values(&f), files);
    assert_eq!(f.count_task("collaboration_tasks").await, 4);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 6);
    f.assert_pool_restored().await;
    drop(workspace);
    f.cleanup().await;
}
