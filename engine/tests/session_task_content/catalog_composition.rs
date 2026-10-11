//! Composed exact metadata stays orthogonal to text content and current private ownership.
use super::catalog as c;
use super::*;

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
        summary: "Explicit metadata registration without assessment".into(),
        evidence_schema: Some(pin(format!("{package}.text"))),
        assessment_policy: Some(pin(format!("{package}.review"))),
    }
}

fn configured(f: &Fixture, values: &[TaskDefinition]) -> SessionCatalogTaskContentWorkspace {
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
    let metadata = TaskDefinitionCatalog::compose(&catalogs).unwrap();
    let workspace = c::workspace(f, &metadata);
    drop((metadata, catalogs));
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
async fn postgres_session_content_catalog_composed_packages_keep_text_versions_and_isolated_admission(
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
    let original = f.create_artifact(token, "\t原文😀\r\n".as_bytes()).await;
    let next = f.create_artifact(token, b"Replacement text\n").await;
    let pins = vec![original.reference.clone(), next.reference.clone()];
    let mut tasks = Vec::new();
    for definition in &values {
        let expected = manifest("Instance labels are independent", pins.clone());
        let task = f
            .auth
            .source
            .create_session_catalog_content_task(
                token,
                &workspace,
                id(),
                &definition.reference,
                expected.clone(),
            )
            .await
            .unwrap();
        assert_eq!(task.task.definition.as_ref(), Some(definition));
        assert_eq!(task.task.owner, f.auth.learner.principal.reference);
        assert_eq!(task.manifest, expected);
        let read = c::get(&f, &workspace, &task).await.unwrap().unwrap();
        assert_eq!(read.snapshot, task);
        assert_eq!(read.contents[0].bytes, "\t原文😀\r\n".as_bytes());
        assert_eq!(read.contents[1].bytes, b"Replacement text\n");
        tasks.push(task);
    }
    let replacement = manifest(
        "Reordered content labels",
        vec![next.reference.clone(), original.reference.clone()],
    );
    let changed = c::replace(&f, &workspace, &tasks[1], replacement.clone())
        .await
        .unwrap();
    let ready = c::transition(&f, &workspace, &changed, TaskStatus::Ready)
        .await
        .unwrap();
    assert_eq!(ready.task.definition.as_ref(), Some(&values[1]));
    assert_eq!(ready.task.created_at, tasks[1].task.created_at);
    assert_eq!(ready.task.owner, tasks[1].task.owner);
    assert_eq!(u64::from(ready.task.revision), 3);
    assert_eq!(ready.manifest, replacement);
    let read = c::get(&f, &workspace, &ready).await.unwrap().unwrap();
    assert_eq!(read.snapshot, ready);
    assert_eq!(read.contents[0].bytes, b"Replacement text\n");
    assert_eq!(read.contents[1].bytes, "\t原文😀\r\n".as_bytes());
    assert_eq!(
        read.contents
            .iter()
            .map(|item| item.revision.reference.clone())
            .collect::<Vec<_>>(),
        ready.manifest.input_refs()
    );

    let target = &tasks[2];
    let mut drifted = values.clone();
    drifted[2].assessment_policy.as_mut().unwrap().version = 3.try_into().unwrap();
    assert_ne!(
        drifted[2], values[2],
        "the fault must change saved metadata"
    );
    let rejected = [
        vec![values[3].clone()],
        vec![values[0].clone(), values[1].clone(), values[3].clone()],
        drifted,
    ];
    let before = c::business_rows(&f).await;
    let files = file_values(&f);
    let invalid = SessionTaskError::Task(TaskStoreError::InvalidRecord);
    for definitions in rejected {
        let selected = configured(&f, &definitions);
        for result in [
            c::get(&f, &selected, target).await.map(|_| ()),
            c::replace(
                &f,
                &selected,
                target,
                manifest("Cannot reinterpret an unknown package", vec![]),
            )
            .await
            .map(|_| ()),
            c::transition(&f, &selected, target, TaskStatus::Ready)
                .await
                .map(|_| ()),
        ] {
            assert_eq!(result, Err(invalid));
        }
        assert_eq!(
            c::get(&f, &selected, &tasks[3])
                .await
                .unwrap()
                .unwrap()
                .snapshot,
            tasks[3]
        );
        assert_eq!(c::business_rows(&f).await, before);
        assert_eq!(file_values(&f), files);
    }
    for mut unknown in [values[0].reference.clone(), values[2].reference.clone()] {
        unknown.version = 3.try_into().unwrap();
        assert_eq!(
            f.auth
                .source
                .create_session_catalog_content_task(
                    token,
                    &workspace,
                    id(),
                    &unknown,
                    manifest("No floating version", vec![])
                )
                .await,
            Err(SessionTaskError::Task(TaskStoreError::UnknownDefinition))
        );
    }
    for task in &tasks {
        assert_eq!(
            f.auth
                .source
                .get_session_catalog_content_task(
                    &f.auth.manager_token,
                    &workspace,
                    task.task.reference.task_id
                )
                .await
                .unwrap(),
            None
        );
    }
    for result in [
        f.auth
            .source
            .replace_session_catalog_content_task(
                &f.auth.manager_token,
                &workspace,
                target.task.reference.task_id,
                target.task.revision,
                manifest("Manager is not content owner", vec![]),
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
    assert_eq!(c::business_rows(&f).await, before);
    assert_eq!(file_values(&f), files);
    assert_eq!(f.count_task("collaboration_tasks").await, 4);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 6);
    assert_eq!(f.files(), 2);
    f.assert_pool_restored().await;
    drop(workspace);
    f.cleanup().await;
}
