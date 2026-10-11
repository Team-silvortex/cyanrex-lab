use super::catalog as c;
use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_catalog_provider_free_versions_empty_and_32_ordered_text_roundtrip(
) {
    let f = Fixture::ready().await;
    let values = c::definitions(vec![c::definition(1), c::definition(2)]);
    let workspace = c::workspace(&f, &values);
    let reopened = workspace.clone();
    drop(values);
    let empty = c::create(&f, &workspace, manifest("Empty catalog task", vec![])).await;
    let mut artifacts = Vec::new();
    for index in 0..32 {
        artifacts.push(
            f.create_artifact(
                &f.auth.learner_token,
                format!("\t内容 {index} 😀\r\n").as_bytes(),
            )
            .await,
        );
    }
    let ordered: Vec<_> = artifacts
        .iter()
        .rev()
        .map(|value| value.reference.clone())
        .collect();
    let task = f
        .auth
        .source
        .create_session_catalog_content_task(
            &f.auth.learner_token,
            &workspace,
            id(),
            &c::definition(2).reference,
            manifest("Thirty-two ordered items", ordered.clone()),
        )
        .await
        .unwrap();
    drop(workspace);
    let read = c::get(&f, &reopened, &task).await.unwrap().unwrap();
    assert_eq!(read.snapshot, task);
    assert_eq!(task.task.definition, Some(c::definition(2)));
    assert_eq!(task.task.input_refs, ordered);
    for (actual, expected) in read.contents.iter().zip(artifacts.iter().rev()) {
        assert_eq!(actual.revision, *expected);
        assert_eq!(actual.bytes, fs::read(f.blob(&expected.reference)).unwrap());
    }
    let reverse = artifacts
        .iter()
        .map(|value| value.reference.clone())
        .collect();
    let changed = c::replace(
        &f,
        &reopened,
        &task,
        manifest("Reordered catalog items", reverse),
    )
    .await
    .unwrap();
    let ready = c::transition(&f, &reopened, &changed, TaskStatus::Ready)
        .await
        .unwrap();
    assert_eq!(ready.task.definition, task.task.definition);
    assert_eq!(ready.task.owner, f.auth.learner.principal.reference);
    assert_eq!(ready.task.created_at, task.task.created_at);
    assert_eq!(u64::from(ready.task.revision), 3);
    let reread = c::get(&f, &reopened, &ready).await.unwrap().unwrap();
    assert_eq!(reread.snapshot, ready);
    assert_eq!(
        reread
            .contents
            .iter()
            .map(|value| value.revision.reference.clone())
            .collect::<Vec<_>>(),
        ready.task.input_refs
    );
    assert_eq!(
        c::get(&f, &reopened, &empty)
            .await
            .unwrap()
            .unwrap()
            .snapshot
            .task
            .definition,
        Some(c::definition(1))
    );
    assert!(c::get(&f, &reopened, &empty)
        .await
        .unwrap()
        .unwrap()
        .contents
        .is_empty());
    assert_eq!(
        f.auth
            .source
            .get_session_catalog_content_task(
                &f.auth.manager_token,
                &reopened,
                task.task.reference.task_id
            )
            .await
            .unwrap(),
        None
    );
    assert_eq!(f.count_task("collaboration_tasks").await, 2);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 4);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 32);
    assert_eq!(f.files(), 32);
    f.assert_pool_restored().await;
    drop(reopened);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_catalog_and_manual_handles_reject_each_other_without_schema_adoption(
) {
    let f = Fixture::ready().await;
    let workspace = c::workspace(&f, &c::definitions(vec![c::definition(1)]));
    let manual = f.create(vec![]).await;
    let catalog = c::create(&f, &workspace, manifest("Catalog-only task", vec![])).await;
    let before = c::business_rows(&f).await;
    let invalid = SessionTaskError::Task(TaskStoreError::InvalidRecord);
    assert_eq!(c::get(&f, &workspace, &manual).await, Err(invalid));
    assert_eq!(
        c::replace(
            &f,
            &workspace,
            &manual,
            manifest("No catalog adoption", vec![])
        )
        .await,
        Err(invalid)
    );
    assert_eq!(
        c::transition(&f, &workspace, &manual, TaskStatus::Ready).await,
        Err(invalid)
    );
    assert_eq!(
        f.get(&f.auth.learner_token, catalog.task.reference.task_id)
            .await,
        Err(invalid)
    );
    assert_eq!(
        f.replace(
            &f.auth.learner_token,
            &catalog,
            manifest("No manual adoption", vec![])
        )
        .await,
        Err(invalid)
    );
    assert_eq!(
        f.transition(&f.auth.learner_token, &catalog, TaskStatus::Ready)
            .await,
        Err(invalid)
    );
    assert_eq!(
        f.tasks
            .get(catalog.task.reference, catalog.task.owner)
            .await,
        Err(TaskStoreError::InvalidRecord)
    );
    let old = SessionTaskWorkspace::new(&f.task_schema, scope()).unwrap();
    assert_eq!(
        f.auth
            .source
            .get_session_manual_task(&f.auth.learner_token, &old, catalog.task.reference.task_id)
            .await,
        Err(SessionTaskError::Task(TaskStoreError::UnsupportedSchema))
    );
    let legacy = SessionCatalogTaskContentWorkspace::from_definitions(
        &f.base.task_schema,
        scope(),
        f.artifact_workspace.clone(),
        &c::definitions(vec![c::definition(1)]),
    )
    .unwrap();
    for result in [
        f.auth
            .source
            .create_session_catalog_content_task(
                &f.auth.learner_token,
                &legacy,
                id(),
                &c::definition(1).reference,
                manifest("No upgrade", vec![]),
            )
            .await
            .map(|_| ()),
        f.auth
            .source
            .get_session_catalog_content_task(
                &f.auth.learner_token,
                &legacy,
                catalog.task.reference.task_id,
            )
            .await
            .map(|_| ()),
        f.auth
            .source
            .replace_session_catalog_content_task(
                &f.auth.learner_token,
                &legacy,
                catalog.task.reference.task_id,
                catalog.task.revision,
                manifest("No migration", vec![]),
            )
            .await
            .map(|_| ()),
        f.auth
            .source
            .transition_session_catalog_content_task(
                &f.auth.learner_token,
                &legacy,
                catalog.task.reference.task_id,
                catalog.task.revision,
                TaskStatus::Ready,
            )
            .await
            .map(|_| ()),
    ] {
        assert_eq!(
            result,
            Err(SessionTaskError::Task(TaskStoreError::UnsupportedSchema))
        );
    }
    assert_eq!(c::business_rows(&f).await, before);
    assert_eq!(f.base.count_task("collaboration_tasks").await, 0);
    assert_eq!(f.files(), 0);
    assert_eq!(
        c::get(&f, &workspace, &catalog)
            .await
            .unwrap()
            .unwrap()
            .snapshot,
        catalog
    );
    f.unchanged(&manual, 2).await;
    f.assert_pool_restored().await;
    drop((workspace, legacy));
    f.cleanup().await;
}
