use super::*;
use cyanrex_engine::services::task_catalog::TaskDefinitionCatalog;

fn definitions(values: Vec<TaskDefinition>) -> TaskDefinitionCatalog {
    TaskDefinitionCatalog::new(definition(1).reference.package, values).unwrap()
}

fn workspace(f: &Fixture, values: &TaskDefinitionCatalog) -> SessionCatalogTaskWorkspace {
    SessionCatalogTaskWorkspace::from_definitions(
        f.task_workspace.clone(),
        f.artifact_workspace.clone(),
        values,
    )
    .unwrap()
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_provider_free_metadata_roundtrip_preserves_inputs_and_versions(
) {
    let f = Fixture::ready().await;
    let definitions = definitions(vec![definition(1), definition(2)]);
    let copied = definitions.clone();
    let workspace = workspace(&f, &definitions);
    drop(definitions);
    let token = &f.auth.learner_token;
    let direct = f
        .tasks
        .create(
            id(),
            f.auth.learner.principal.reference,
            TaskDraft::from_definitions(&copied, &reference(), "Trusted metadata draft", vec![])
                .unwrap(),
        )
        .await
        .unwrap();
    let empty = f
        .auth
        .source
        .create_session_catalog_task(token, &workspace, id(), &reference(), "Empty work", vec![])
        .await
        .unwrap();
    let binary = f.create_artifact(token, &[0, 255, 1]).await;
    let text = f.create_artifact(token, b"A different input").await;
    let original = vec![text.reference.clone(), binary.reference.clone()];
    let task = f
        .auth
        .source
        .create_session_catalog_task(
            token,
            &workspace,
            id(),
            &definition(2).reference,
            "Metadata-only admission",
            original.clone(),
        )
        .await
        .unwrap();
    let before = f
        .auth
        .source
        .get_session_catalog_task(token, &workspace, task.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    let changed = f
        .auth
        .source
        .replace_session_catalog_task_inputs(
            token,
            &workspace,
            task.reference.task_id,
            task.revision,
            vec![binary.reference.clone(), text.reference.clone()],
        )
        .await
        .unwrap();
    let ready = f
        .auth
        .source
        .transition_session_catalog_task(
            token,
            &workspace,
            task.reference.task_id,
            changed.revision,
            TaskStatus::Ready,
        )
        .await
        .unwrap();
    let reopened = SessionCatalogTaskWorkspace::from_definitions(
        f.task_workspace.clone(),
        f.artifact_workspace.clone(),
        &copied,
    )
    .unwrap();
    drop(copied);
    let after = f
        .auth
        .source
        .get_session_catalog_task(token, &reopened, ready.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    // The legacy provider-backed constructor resolves the same stored version-one definition.
    let compatible_empty = f
        .get(token, empty.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    let direct_read = f
        .auth
        .source
        .get_session_catalog_task(token, &reopened, direct.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    let foreign = f
        .auth
        .source
        .get_session_catalog_task(&f.auth.manager_token, &reopened, task.reference.task_id)
        .await
        .unwrap();
    let counts = (
        f.count_task("collaboration_tasks").await,
        f.count_task("collaboration_task_outbox").await,
        f.count_artifact("collaboration_artifact_outbox").await,
        f.files(),
    );
    let owner = f.auth.learner.principal.reference;
    f.assert_pool_restored().await;
    drop((workspace, reopened));
    f.cleanup().await;

    assert_eq!(empty.owner, owner);
    assert_eq!(direct.definition, Some(definition(1)));
    assert_eq!(direct.title, "Trusted metadata draft");
    assert_eq!(direct_read.task, direct);
    assert!(direct_read.inputs.is_empty());
    assert_eq!(empty.definition, Some(definition(1)));
    assert_eq!(compatible_empty.task, empty);
    assert!(compatible_empty.inputs.is_empty());
    assert_eq!(task.definition, Some(definition(2)));
    assert_eq!(task.owner, owner);
    assert_eq!(task.input_refs, original);
    assert_eq!(before.task, task);
    assert_eq!(before.inputs[0].bytes, b"A different input");
    assert_eq!(before.inputs[1].bytes, [0, 255, 1]);
    assert_eq!(changed.revision, 2.try_into().unwrap());
    assert_eq!(ready.revision, 3.try_into().unwrap());
    assert_eq!(ready.status, TaskStatus::Ready);
    assert_eq!(ready.definition, task.definition);
    assert_eq!(after.task, ready);
    assert_eq!(after.inputs[0].bytes, [0, 255, 1]);
    assert_eq!(after.inputs[1].bytes, b"A different input");
    assert_eq!(foreign, None);
    assert_eq!(counts, (3, 5, 2, 2));
}

async fn business_rows(f: &Fixture) -> Vec<Vec<String>> {
    let mut snapshots = Vec::new();
    for (pool, table) in [
        (&f.task_pool, "collaboration_tasks"),
        (&f.task_pool, "collaboration_task_outbox"),
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
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_provider_free_metadata_drift_and_unknown_pins_never_write()
{
    let f = Fixture::ready().await;
    let original = definitions(vec![definition(1)]);
    let original_workspace = workspace(&f, &original);
    let token = &f.auth.learner_token;
    let artifact = f.create_artifact(token, b"Immutable original bytes").await;
    let task = f
        .auth
        .source
        .create_session_catalog_task(
            token,
            &original_workspace,
            id(),
            &reference(),
            "Exact admitted definition",
            vec![artifact.reference.clone()],
        )
        .await
        .unwrap();
    let before = business_rows(&f).await;
    let mut errors = Vec::new();
    for field in 0..6 {
        let mut changed = definition(1);
        match field {
            0 => changed.title = "Changed title".into(),
            1 => changed.summary = "Changed summary".into(),
            2 => {
                changed.evidence_schema.as_mut().unwrap().name =
                    "sample.document.other".parse().unwrap()
            }
            3 => changed.evidence_schema.as_mut().unwrap().version = 2.try_into().unwrap(),
            4 => {
                changed.assessment_policy.as_mut().unwrap().name =
                    "sample.document.other".parse().unwrap()
            }
            5 => changed.assessment_policy.as_mut().unwrap().version = 2.try_into().unwrap(),
            _ => unreachable!(),
        }
        let changed_workspace = workspace(&f, &definitions(vec![changed]));
        errors.push(
            f.auth
                .source
                .get_session_catalog_task(token, &changed_workspace, task.reference.task_id)
                .await
                .map(|_| ()),
        );
        errors.push(
            f.auth
                .source
                .transition_session_catalog_task(
                    token,
                    &changed_workspace,
                    task.reference.task_id,
                    task.revision,
                    TaskStatus::Ready,
                )
                .await
                .map(|_| ()),
        );
        errors.push(
            f.auth
                .source
                .replace_session_catalog_task_inputs(
                    token,
                    &changed_workspace,
                    task.reference.task_id,
                    task.revision,
                    vec![],
                )
                .await
                .map(|_| ()),
        );
    }
    let missing_workspace = workspace(&f, &definitions(vec![definition(2)]));
    let unknown = TaskStoreError::UnknownDefinition;
    let missing = vec![
        f.auth
            .source
            .get_session_catalog_task(token, &missing_workspace, task.reference.task_id)
            .await
            .map(|_| ()),
        f.auth
            .source
            .transition_session_catalog_task(
                token,
                &missing_workspace,
                task.reference.task_id,
                task.revision,
                TaskStatus::Ready,
            )
            .await
            .map(|_| ()),
        f.auth
            .source
            .replace_session_catalog_task_inputs(
                token,
                &missing_workspace,
                task.reference.task_id,
                task.revision,
                vec![],
            )
            .await
            .map(|_| ()),
        f.auth
            .source
            .create_session_catalog_task(
                token,
                &missing_workspace,
                id(),
                &reference(),
                "No fallback",
                vec![],
            )
            .await
            .map(|_| ()),
    ];
    let retained = f
        .auth
        .source
        .get_session_catalog_task(token, &original_workspace, task.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    let after = business_rows(&f).await;
    let blob = fs::read(f.blob(&artifact.reference)).unwrap();
    let files = f.files();
    f.assert_pool_restored().await;
    drop((original_workspace, missing_workspace));
    f.cleanup().await;

    assert_eq!(errors, vec![Err(SessionTaskError::DefinitionMismatch); 18]);
    assert_eq!(missing, vec![Err(SessionTaskError::Task(unknown)); 4]);
    assert_eq!(before, after);
    assert_eq!(retained.task, task);
    assert_eq!(retained.inputs[0].bytes, blob);
    assert_eq!(blob, b"Immutable original bytes");
    assert_eq!(files, 1);
}
