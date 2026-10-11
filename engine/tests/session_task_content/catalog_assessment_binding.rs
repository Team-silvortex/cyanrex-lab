//! Unbound automatic assessment does not relax exact catalogue/content/Session boundaries.
use super::catalog as c;
use super::*;

fn configured(f: &Fixture, values: Vec<TaskDefinition>) -> SessionCatalogTaskContentWorkspace {
    let metadata = TaskDefinitionCatalog::new(values[0].reference.package.clone(), values).unwrap();
    let workspace = c::workspace(f, &metadata);
    drop(metadata);
    workspace
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
async fn postgres_session_content_catalog_unbound_assessment_roundtrips_and_binding_drift_never_writes(
) {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let mut unbound = c::definition(1);
    unbound.evidence_schema = None;
    unbound.assessment_policy = None;
    let bound = c::definition(2);
    assert!(bound.evidence_schema.is_some() && bound.assessment_policy.is_some());
    let metadata = c::definitions(vec![unbound.clone(), bound.clone()]);
    let workspace = c::workspace(&f, &metadata);
    let store = TaskContentStore::from_definitions(f.task_pool.clone(), scope(), &metadata);
    drop(metadata);
    let original = f.create_artifact(token, "原文😀\r\n".as_bytes()).await;
    let replacement = f.create_artifact(token, b"Replacement text\n").await;
    // Trusted storage and Session decoding use the same explicit definition mode.
    let empty = store
        .create_catalog(
            id(),
            f.auth.learner.principal.reference,
            &unbound.reference,
            manifest("Unbound empty task", vec![]),
        )
        .await
        .unwrap();
    let bound_task = store
        .create_catalog(
            id(),
            f.auth.learner.principal.reference,
            &bound.reference,
            manifest("Bound storage task", vec![original.reference.clone()]),
        )
        .await
        .unwrap();
    for target in [&empty, &bound_task] {
        assert_eq!(
            store
                .get(target.task.reference, target.task.owner)
                .await
                .unwrap(),
            Some(target.clone())
        );
        assert_eq!(
            c::get(&f, &workspace, target)
                .await
                .unwrap()
                .unwrap()
                .snapshot,
            *target
        );
    }
    assert_eq!(empty.task.definition, Some(unbound.clone()));
    assert_eq!(bound_task.task.definition, Some(bound.clone()));
    assert!(c::get(&f, &workspace, &empty)
        .await
        .unwrap()
        .unwrap()
        .contents
        .is_empty());
    let task = f
        .auth
        .source
        .create_session_catalog_content_task(
            token,
            &workspace,
            id(),
            &unbound.reference,
            manifest("Unbound text task", vec![original.reference.clone()]),
        )
        .await
        .unwrap();
    assert_eq!(task.task.definition, Some(unbound.clone()));
    let observed = c::get(&f, &workspace, &task).await.unwrap().unwrap();
    assert_eq!(observed.snapshot, task);
    assert_eq!(observed.contents[0].bytes, "原文😀\r\n".as_bytes());
    let replacement_manifest = manifest(
        "Revised human work",
        vec![replacement.reference.clone(), original.reference.clone()],
    );
    let changed = c::replace(&f, &workspace, &task, replacement_manifest.clone())
        .await
        .unwrap();
    let ready = c::transition(&f, &workspace, &changed, TaskStatus::Ready)
        .await
        .unwrap();
    assert_eq!(ready.task.definition, Some(unbound.clone()));
    assert_eq!(ready.task.owner, f.auth.learner.principal.reference);
    assert_eq!(ready.task.created_at, task.task.created_at);
    assert_eq!(u64::from(ready.task.revision), 3);
    assert_eq!(ready.manifest, replacement_manifest);
    let read = c::get(&f, &workspace, &ready).await.unwrap().unwrap();
    assert_eq!(read.snapshot, ready);
    assert_eq!(read.contents[0].bytes, b"Replacement text\n");
    assert_eq!(read.contents[1].bytes, "原文😀\r\n".as_bytes());
    let manual = f.create(vec![]).await;
    let before = c::business_rows(&f).await;
    let files = file_values(&f);
    let invalid = SessionTaskError::Task(TaskStoreError::InvalidRecord);
    assert_eq!(
        f.get(token, empty.task.reference.task_id).await,
        Err(invalid)
    );
    assert_eq!(c::get(&f, &workspace, &manual).await, Err(invalid));
    assert_eq!(
        f.tasks.get(empty.task.reference, empty.task.owner).await,
        Err(TaskStoreError::InvalidRecord)
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
        assert_eq!(target.task.status, TaskStatus::Draft);
        let selected = configured(&f, definitions.clone());
        let selected_metadata = c::definitions(definitions);
        let selected_store =
            TaskContentStore::from_definitions(f.task_pool.clone(), scope(), &selected_metadata);
        drop(selected_metadata);
        let changed_manifest = manifest(
            "Cannot erase binding drift",
            vec![replacement.reference.clone()],
        );
        for result in [
            c::get(&f, &selected, target).await.map(|_| ()),
            c::replace(&f, &selected, target, changed_manifest.clone())
                .await
                .map(|_| ()),
            c::transition(&f, &selected, target, TaskStatus::Ready)
                .await
                .map(|_| ()),
        ] {
            assert_eq!(result, Err(invalid));
        }
        for result in [
            selected_store
                .get(target.task.reference, target.task.owner)
                .await
                .map(|_| ()),
            selected_store
                .replace(
                    target.task.reference,
                    target.task.owner,
                    target.task.revision,
                    changed_manifest,
                )
                .await
                .map(|_| ()),
            selected_store
                .transition(
                    target.task.reference,
                    target.task.owner,
                    target.task.revision,
                    TaskStatus::Ready,
                )
                .await
                .map(|_| ()),
        ] {
            assert_eq!(result, Err(TaskStoreError::InvalidRecord));
        }
        assert_eq!(
            c::get(&f, &selected, unaffected)
                .await
                .unwrap()
                .unwrap()
                .snapshot,
            *unaffected
        );
        assert_eq!(
            selected_store
                .get(unaffected.task.reference, unaffected.task.owner)
                .await
                .unwrap(),
            Some(unaffected.clone())
        );
        assert_eq!(c::business_rows(&f).await, before);
        assert_eq!(file_values(&f), files);
    }
    for target in [&empty, &bound_task] {
        assert_eq!(
            f.auth
                .source
                .get_session_catalog_content_task(
                    &f.auth.manager_token,
                    &workspace,
                    target.task.reference.task_id
                )
                .await
                .unwrap(),
            None
        );
        for result in [
            f.auth
                .source
                .replace_session_catalog_content_task(
                    &f.auth.manager_token,
                    &workspace,
                    target.task.reference.task_id,
                    target.task.revision,
                    manifest(
                        "Manager is not content owner",
                        vec![replacement.reference.clone()],
                    ),
                )
                .await
                .map(|_| ()),
            f.auth
                .source
                .transition_session_catalog_content_task(
                    &f.auth.manager_token,
                    &workspace,
                    target.task.reference.task_id,
                    target.task.revision,
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
    assert_eq!(c::business_rows(&f).await, before);
    assert_eq!(file_values(&f), files);
    assert_eq!(f.count_task("collaboration_tasks").await, 4);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 6);
    f.assert_pool_restored().await;
    drop((workspace, store));
    f.cleanup().await;
}
