use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_reads_ordered_unicode_revisions_and_empty_tasks() {
    let f = Fixture::ready().await;
    let first = f
        .create_artifact(&f.auth.learner_token, "\t中文😀\r\n".as_bytes())
        .await;
    let second = f
        .auth
        .source
        .revise_session_artifact(
            &f.auth.learner_token,
            &f.artifact_workspace,
            &first.reference,
            id(),
            input_fixture::draft(b"New revision"),
        )
        .await
        .unwrap();
    let task = f
        .create(vec![second.reference.clone(), first.reference.clone()])
        .await;
    assert_eq!(task.task.owner, f.auth.learner.principal.reference);
    assert!(task.task.definition.is_none());
    let read = f
        .get(&f.auth.learner_token, task.task.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.snapshot, task);
    assert_eq!(
        read.contents
            .iter()
            .map(|value| value.revision.reference.clone())
            .collect::<Vec<_>>(),
        vec![second.reference, first.reference]
    );
    assert_eq!(read.contents[0].bytes, b"New revision");
    assert_eq!(read.contents[1].bytes, "\t中文😀\r\n".as_bytes());
    let empty = f.create(vec![]).await;
    let read = f
        .get(&f.auth.learner_token, empty.task.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.snapshot, empty);
    assert!(read.contents.is_empty());
    assert_eq!(f.files(), 2);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_metadata_edits_and_status_share_exact_revision_checks() {
    let (f, original, old, new) = pair().await;
    let edited = TaskContentManifest::new(
        "Renamed",
        vec![TextPayloadBinding::new(old.reference.clone(), "changed.rs", "rust").unwrap()],
    )
    .unwrap();
    let changed = f
        .replace(&f.auth.learner_token, &original, edited.clone())
        .await
        .unwrap();
    assert_eq!(changed.manifest, edited);
    assert_eq!(u64::from(changed.task.revision), 2);
    assert_eq!(changed.task.created_at, original.task.created_at);
    assert_eq!(
        f.replace(&f.auth.learner_token, &original, manifest("Stale", vec![]))
            .await,
        Err(SessionTaskError::Task(TaskStoreError::StaleRevision))
    );
    assert_eq!(
        f.replace(&f.auth.learner_token, &changed, changed.manifest.clone())
            .await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
    );
    let ordered = f
        .replace(
            &f.auth.learner_token,
            &changed,
            manifest(
                "Ordered",
                vec![new.reference.clone(), old.reference.clone()],
            ),
        )
        .await
        .unwrap();
    let empty = f
        .replace(&f.auth.learner_token, &ordered, manifest("Empty", vec![]))
        .await
        .unwrap();
    let ready = f
        .transition(&f.auth.learner_token, &empty, TaskStatus::Ready)
        .await
        .unwrap();
    assert_eq!(ready.manifest, empty.manifest);
    assert_eq!(u64::from(ready.task.revision), 5);
    assert_eq!(
        f.replace(&f.auth.learner_token, &ready, manifest("Not draft", vec![]))
            .await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidTransition))
    );
    f.unchanged(&ready, 5).await;
    assert_eq!(f.files(), 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_private_ownership_scope_and_forged_digests_never_grant_access() {
    let (f, task, old, new) = pair().await;
    assert_eq!(
        f.get(&f.auth.manager_token, task.task.reference.task_id)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        f.replace(&f.auth.manager_token, &task, manifest("No sharing", vec![]))
            .await,
        Err(SessionTaskError::Task(TaskStoreError::NotFound))
    );
    let foreign = f
        .create_artifact(&f.auth.manager_token, b"Private manager content")
        .await;
    let mut digest = old.reference.clone();
    digest.sha256 = "b".repeat(64).parse().unwrap();
    let mut scope_mismatch = old.reference.clone();
    scope_mismatch.workspace.workspace_id = id();
    for refs in [vec![foreign.reference], vec![digest], vec![scope_mismatch]] {
        let requested = manifest("Invalid references", refs);
        assert!(f
            .request(&f.auth.learner_token, id(), requested.clone())
            .await
            .is_err());
        assert!(f
            .replace(&f.auth.learner_token, &task, requested)
            .await
            .is_err());
    }
    let mut conflicting = new.reference.clone();
    conflicting.sha256 = "c".repeat(64).parse().unwrap();
    let next = f
        .replace(
            &f.auth.learner_token,
            &task,
            manifest("New", vec![new.reference.clone()]),
        )
        .await
        .unwrap();
    assert!(f
        .replace(
            &f.auth.learner_token,
            &next,
            manifest("Same coordinates, forged", vec![conflicting])
        )
        .await
        .is_err());
    f.unchanged(&next, 2).await;
    assert_eq!(f.files(), 3);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_invalid_new_text_never_creates_or_replaces_a_task() {
    let f = Fixture::ready().await;
    let original = f.create(vec![]).await;
    for bytes in [
        vec![0xff],
        b"bad\0text".to_vec(),
        "bad\u{85}text".as_bytes().to_vec(),
        vec![b'x'; 256 * 1024 + 1],
    ] {
        let artifact = f.create_artifact(&f.auth.learner_token, &bytes).await;
        let requested = manifest("Not valid text", vec![artifact.reference]);
        assert_eq!(
            f.request(&f.auth.learner_token, id(), requested.clone())
                .await,
            Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
        );
        assert_eq!(
            f.replace(&f.auth.learner_token, &original, requested).await,
            Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
        );
        f.unchanged(&original, 1).await;
    }
    let edge = f
        .create_artifact(&f.auth.learner_token, &vec![b'x'; 256 * 1024])
        .await;
    let valid = f
        .replace(
            &f.auth.learner_token,
            &original,
            manifest("Exact limit", vec![edge.reference]),
        )
        .await
        .unwrap();
    assert_eq!(
        f.get(&f.auth.learner_token, valid.task.reference.task_id)
            .await
            .unwrap()
            .unwrap()
            .contents[0]
            .bytes
            .len(),
        256 * 1024
    );
    assert_eq!(f.files(), 5);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_schema_three_and_legacy_session_adapters_reject_each_other() {
    let f = Fixture::ready().await;
    let empty = f.create(vec![]).await;
    let old_workspace = SessionTaskWorkspace::new(&f.task_schema, scope()).unwrap();
    assert_eq!(
        f.auth
            .source
            .get_session_manual_task(
                &f.auth.learner_token,
                &old_workspace,
                empty.task.reference.task_id
            )
            .await,
        Err(SessionTaskError::Task(TaskStoreError::UnsupportedSchema))
    );
    assert_eq!(
        f.auth
            .source
            .create_session_manual_task(&f.auth.learner_token, &old_workspace, id(), "Legacy")
            .await,
        Err(SessionTaskError::Task(TaskStoreError::UnsupportedSchema))
    );
    let old_inputs =
        SessionTaskInputsWorkspace::new(old_workspace, f.artifact_workspace.clone()).unwrap();
    assert_eq!(
        f.auth
            .source
            .get_session_input_task(
                &f.auth.learner_token,
                &old_inputs,
                empty.task.reference.task_id
            )
            .await,
        Err(SessionTaskError::Task(TaskStoreError::UnsupportedSchema))
    );
    let new_on_legacy = SessionTaskContentWorkspace::new(
        &f.base.task_schema,
        scope(),
        f.artifact_workspace.clone(),
    )
    .unwrap();
    assert_eq!(
        f.auth
            .source
            .create_session_content_task(
                &f.auth.learner_token,
                &new_on_legacy,
                id(),
                manifest("No migration", vec![])
            )
            .await,
        Err(SessionTaskError::Task(TaskStoreError::UnsupportedSchema))
    );
    f.unchanged(&empty, 1).await;
    assert_eq!(f.base.count_task("collaboration_tasks").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_empty_payload_still_requires_valid_artifact_metadata() {
    let f = Fixture::ready().await;
    let original = f.create(vec![]).await;
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 99")
        .await;
    assert!(f
        .request(&f.auth.learner_token, id(), manifest("Empty", vec![]))
        .await
        .is_err());
    assert!(f
        .get(&f.auth.learner_token, original.task.reference.task_id)
        .await
        .is_err());
    assert!(f
        .replace(
            &f.auth.learner_token,
            &original,
            manifest("Still empty", vec![])
        )
        .await
        .is_err());
    assert!(f
        .transition(&f.auth.learner_token, &original, TaskStatus::Ready)
        .await
        .is_err());
    f.unchanged(&original, 1).await;
    assert_eq!(f.files(), 0);
    f.cleanup().await;
}
