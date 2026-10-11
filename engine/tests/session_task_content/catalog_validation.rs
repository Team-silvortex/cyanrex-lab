use super::catalog as c;
use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_catalog_full_metadata_drift_and_unknown_pins_have_no_side_effects(
) {
    let (f, original, task, old, new) = c::pair().await;
    let before = c::business_rows(&f).await;
    for field in 0..6 {
        let mut changed = c::definition(1);
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
        let changed = c::workspace(&f, &c::definitions(vec![changed]));
        for result in [
            c::get(&f, &changed, &task).await.map(|_| ()),
            c::replace(
                &f,
                &changed,
                &task,
                manifest("Cannot erase metadata drift", vec![new.reference.clone()]),
            )
            .await
            .map(|_| ()),
            c::transition(&f, &changed, &task, TaskStatus::Ready)
                .await
                .map(|_| ()),
        ] {
            assert_eq!(
                result,
                Err(SessionTaskError::Task(TaskStoreError::InvalidRecord)),
                "field {field}"
            );
        }
    }
    let missing = c::workspace(&f, &c::definitions(vec![c::definition(2)]));
    for result in [
        c::get(&f, &missing, &task).await.map(|_| ()),
        c::replace(
            &f,
            &missing,
            &task,
            manifest("Cannot rewrite unknown definition", vec![]),
        )
        .await
        .map(|_| ()),
        c::transition(&f, &missing, &task, TaskStatus::Cancelled)
            .await
            .map(|_| ()),
    ] {
        assert_eq!(
            result,
            Err(SessionTaskError::Task(TaskStoreError::InvalidRecord))
        );
    }
    assert_eq!(
        f.auth
            .source
            .create_session_catalog_content_task(
                &f.auth.learner_token,
                &missing,
                id(),
                &c::definition(1).reference,
                manifest("No latest-version fallback", vec![]),
            )
            .await,
        Err(SessionTaskError::Task(TaskStoreError::UnknownDefinition))
    );
    assert_eq!(c::business_rows(&f).await, before);
    let retained = c::get(&f, &original, &task).await.unwrap().unwrap();
    assert_eq!(retained.snapshot, task);
    assert_eq!(retained.contents[0].bytes, b"Original input");
    assert_eq!(fs::read(f.blob(&old.reference)).unwrap(), b"Original input");
    assert_eq!(
        fs::read(f.blob(&new.reference)).unwrap(),
        b"Replacement input"
    );
    assert_eq!(f.files(), 2);
    f.assert_pool_restored().await;
    drop((original, missing));
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_catalog_old_bytes_new_text_and_revision_guards_cannot_be_bypassed(
) {
    for missing in [false, true] {
        let (f, workspace, task, old, new) = c::pair().await;
        let path = f.blob(&old.reference);
        if missing {
            fs::remove_file(&path).unwrap();
        } else {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            fs::write(&path, b"Corrupted text").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        }
        let before = c::business_rows(&f).await;
        let failure = SessionTaskError::Artifact(ArtifactStoreError::BlobUnavailable);
        assert_eq!(c::get(&f, &workspace, &task).await, Err(failure));
        for next in [
            manifest("Labels only", vec![old.reference.clone()]),
            manifest("Remove corrupt last item", vec![]),
            manifest("Replace corrupt item", vec![new.reference.clone()]),
        ] {
            assert_eq!(c::replace(&f, &workspace, &task, next).await, Err(failure));
        }
        assert_eq!(
            c::transition(&f, &workspace, &task, TaskStatus::Cancelled).await,
            Err(failure)
        );
        assert_eq!(c::business_rows(&f).await, before);
        assert_eq!(f.files(), if missing { 1 } else { 2 });
        assert_eq!(
            f.artifacts
                .read(&new.reference, new.owner)
                .await
                .unwrap()
                .unwrap()
                .bytes,
            b"Replacement input"
        );
        f.assert_pool_restored().await;
        drop(workspace);
        f.cleanup().await;
    }
    let f = Fixture::ready().await;
    let values = c::definitions(vec![c::definition(1)]);
    let workspace = c::workspace(&f, &values);
    let binary = f.create_artifact(&f.auth.learner_token, &[0xff]).await;
    // Trusted metadata storage may pin arbitrary Artifact bytes; the Session adapter may not.
    let store = TaskContentStore::from_definitions(f.task_pool.clone(), scope(), &values);
    let task = store
        .create_catalog(
            id(),
            f.auth.learner.principal.reference,
            &c::definition(1).reference,
            manifest("Trusted but not text", vec![binary.reference]),
        )
        .await
        .unwrap();
    let before = c::business_rows(&f).await;
    for result in [
        c::get(&f, &workspace, &task).await.map(|_| ()),
        c::replace(
            &f,
            &workspace,
            &task,
            manifest("Cannot remove binary validation", vec![]),
        )
        .await
        .map(|_| ()),
        c::transition(&f, &workspace, &task, TaskStatus::Cancelled)
            .await
            .map(|_| ()),
    ] {
        assert_eq!(
            result,
            Err(SessionTaskError::Task(TaskStoreError::InvalidRecord))
        );
    }
    assert_eq!(c::business_rows(&f).await, before);
    assert_eq!(
        store
            .get(task.task.reference, task.task.owner)
            .await
            .unwrap(),
        Some(task)
    );
    assert_eq!(f.files(), 1);
    f.assert_pool_restored().await;
    drop((workspace, store));
    f.cleanup().await;

    let (f, workspace, task, _, new) = c::pair().await;
    for bytes in [
        vec![0xff],
        b"bad\0text".to_vec(),
        "bad\u{85}text".as_bytes().to_vec(),
        vec![b'x'; 256 * 1024 + 1],
    ] {
        let invalid = f.create_artifact(&f.auth.learner_token, &bytes).await;
        let before = c::business_rows(&f).await;
        let value = manifest("Not valid text", vec![invalid.reference]);
        assert_eq!(
            c::replace(&f, &workspace, &task, value.clone()).await,
            Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
        );
        assert_eq!(
            f.auth
                .source
                .create_session_catalog_content_task(
                    &f.auth.learner_token,
                    &workspace,
                    id(),
                    &c::definition(1).reference,
                    value
                )
                .await,
            Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
        );
        assert_eq!(c::business_rows(&f).await, before);
    }
    let foreign = f
        .create_artifact(&f.auth.manager_token, b"Private manager content")
        .await;
    let mut forged = new.reference.clone();
    forged.sha256 = "b".repeat(64).parse().unwrap();
    let mut other_scope = new.reference.clone();
    other_scope.workspace.workspace_id = id();
    for reference in [foreign.reference, forged, other_scope] {
        let before = c::business_rows(&f).await;
        assert!(c::replace(
            &f,
            &workspace,
            &task,
            manifest("Forbidden pin", vec![reference])
        )
        .await
        .is_err());
        assert_eq!(c::business_rows(&f).await, before);
    }
    let changed = c::replace(
        &f,
        &workspace,
        &task,
        manifest("Valid replacement", vec![new.reference]),
    )
    .await
    .unwrap();
    let before = c::business_rows(&f).await;
    assert_eq!(
        c::replace(&f, &workspace, &task, manifest("Stale write", vec![])).await,
        Err(SessionTaskError::Task(TaskStoreError::StaleRevision))
    );
    assert_eq!(
        c::transition(&f, &workspace, &task, TaskStatus::Ready).await,
        Err(SessionTaskError::Task(TaskStoreError::StaleRevision))
    );
    assert_eq!(
        c::replace(&f, &workspace, &changed, changed.manifest.clone()).await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
    );
    assert_eq!(c::business_rows(&f).await, before);
    let ready = c::transition(&f, &workspace, &changed, TaskStatus::Ready)
        .await
        .unwrap();
    let before = c::business_rows(&f).await;
    assert_eq!(
        c::replace(&f, &workspace, &ready, manifest("Not Draft", vec![])).await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidTransition))
    );
    assert_eq!(c::business_rows(&f).await, before);
    assert_eq!(
        c::get(&f, &workspace, &ready)
            .await
            .unwrap()
            .unwrap()
            .snapshot,
        ready
    );
    assert_eq!(ready.task.definition, task.task.definition);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 3);
    assert_eq!(f.files(), 7);
    f.assert_pool_restored().await;
    drop(workspace);
    f.cleanup().await;
}
