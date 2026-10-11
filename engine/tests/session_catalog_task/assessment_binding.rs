//! Definition metadata without automatic assessment remains a catalogue Task, not manual work.
use super::*;
use cyanrex_engine::services::task_catalog::TaskDefinitionCatalog;

fn configured(f: &Fixture, values: Vec<TaskDefinition>) -> SessionCatalogTaskWorkspace {
    let metadata = TaskDefinitionCatalog::new(values[0].reference.package.clone(), values).unwrap();
    let workspace = SessionCatalogTaskWorkspace::from_definitions(
        f.task_workspace.clone(),
        f.artifact_workspace.clone(),
        &metadata,
    )
    .unwrap();
    drop(metadata);
    workspace
}

async fn create(
    f: &Fixture,
    workspace: &SessionCatalogTaskWorkspace,
    definition: &TaskDefinition,
    inputs: Vec<ArtifactRef>,
) -> TaskSnapshot {
    f.auth
        .source
        .create_session_catalog_task(
            &f.auth.learner_token,
            workspace,
            id(),
            &definition.reference,
            "Human-directed work without automatic assessment",
            inputs,
        )
        .await
        .unwrap()
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
async fn postgres_session_catalog_tasks_unbound_assessment_metadata_roundtrips_and_binding_drift_never_writes(
) {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let mut unbound = definition(1);
    unbound.evidence_schema = None;
    unbound.assessment_policy = None;
    let bound = definition(2);
    assert!(bound.evidence_schema.is_some() && bound.assessment_policy.is_some());
    let workspace = configured(&f, vec![unbound.clone(), bound.clone()]);
    let original = f.create_artifact(token, b"Original document\n").await;
    let replacement = f.create_artifact(token, &[0, 255, 1]).await;
    let empty = create(&f, &workspace, &unbound, vec![]).await;
    let task = create(&f, &workspace, &unbound, vec![original.reference.clone()]).await;
    let bound_task = create(&f, &workspace, &bound, vec![original.reference.clone()]).await;
    assert_eq!(empty.definition, Some(unbound.clone()));
    assert_eq!(task.definition, Some(unbound.clone()));
    let read = f
        .auth
        .source
        .get_session_catalog_task(token, &workspace, task.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.task, task);
    assert_eq!(read.inputs[0].bytes, b"Original document\n");
    let changed = f
        .auth
        .source
        .replace_session_catalog_task_inputs(
            token,
            &workspace,
            task.reference.task_id,
            task.revision,
            vec![replacement.reference.clone(), original.reference.clone()],
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
    assert_eq!(ready.definition, Some(unbound.clone()));
    assert_eq!(ready.owner, f.auth.learner.principal.reference);
    assert_eq!(ready.created_at, task.created_at);
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
    assert_eq!(read.inputs[1].bytes, b"Original document\n");
    let observed_empty = f
        .auth
        .source
        .get_session_catalog_task(token, &workspace, empty.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observed_empty.task, empty);
    assert!(observed_empty.inputs.is_empty());

    let manual = f
        .tasks
        .create(
            id(),
            task.owner,
            TaskDraft::manual("Manual work", vec![original.reference.clone()]).unwrap(),
        )
        .await
        .unwrap();
    let manual_inputs =
        SessionTaskInputsWorkspace::new(f.task_workspace.clone(), f.artifact_workspace.clone())
            .unwrap();
    let before = business_rows(&f).await;
    let files = file_values(&f);
    assert_eq!(
        f.auth
            .source
            .get_session_input_task(token, &manual_inputs, ready.reference.task_id)
            .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    assert_eq!(
        f.auth
            .source
            .get_session_manual_task(token, &f.task_workspace, empty.reference.task_id)
            .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    assert_eq!(
        f.auth
            .source
            .get_session_catalog_task(token, &workspace, manual.reference.task_id)
            .await,
        Err(SessionTaskError::UnsupportedTask)
    );

    let mut added = unbound.clone();
    added.evidence_schema = bound.evidence_schema.clone();
    added.assessment_policy = bound.assessment_policy.clone();
    let mut removed = bound.clone();
    removed.evidence_schema = None;
    removed.assessment_policy = None;
    assert_ne!(
        added, unbound,
        "adding a binding must actually change metadata"
    );
    assert_ne!(
        removed, bound,
        "removing a binding must actually change metadata"
    );
    for (target, definitions, unaffected) in [
        (&empty, vec![added, bound.clone()], &bound_task),
        (&bound_task, vec![unbound.clone(), removed], &empty),
    ] {
        assert_eq!(target.status, TaskStatus::Draft);
        let selected = configured(&f, definitions);
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
                    vec![replacement.reference.clone()],
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
            assert_eq!(result, Err(SessionTaskError::DefinitionMismatch));
        }
        assert_eq!(
            f.auth
                .source
                .get_session_catalog_task(token, &selected, unaffected.reference.task_id)
                .await
                .unwrap()
                .unwrap()
                .task,
            *unaffected
        );
        assert_eq!(business_rows(&f).await, before);
        assert_eq!(file_values(&f), files);
    }
    for target in [&empty, &bound_task] {
        assert_eq!(
            f.auth
                .source
                .get_session_catalog_task(
                    &f.auth.manager_token,
                    &workspace,
                    target.reference.task_id
                )
                .await
                .unwrap(),
            None
        );
        for result in [
            f.auth
                .source
                .replace_session_catalog_task_inputs(
                    &f.auth.manager_token,
                    &workspace,
                    target.reference.task_id,
                    target.revision,
                    vec![replacement.reference.clone()],
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
    }
    assert_eq!(business_rows(&f).await, before);
    assert_eq!(file_values(&f), files);
    assert_eq!(f.count_task("collaboration_tasks").await, 4);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 6);
    f.assert_pool_restored().await;
    drop((workspace, manual_inputs));
    f.cleanup().await;
}
