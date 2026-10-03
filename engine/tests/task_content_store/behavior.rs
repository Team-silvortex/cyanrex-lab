use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_keeps_32_ordered_exact_pins_and_duplicate_filename_labels() {
    let f = Fixture::ready().await;
    let pins: Vec<_> = (0..32)
        .map(|_| ArtifactRef {
            revision_id: id(),
            ..artifact()
        })
        .collect();
    let desired = content("Thirty-two independent revisions", pins.clone());
    let saved = f
        .store
        .create(reference().task_id, owner(), desired.clone())
        .await
        .unwrap();
    assert_eq!(saved.manifest, desired);
    assert_eq!(saved.task.input_refs, pins);
    assert!(saved
        .manifest
        .payload()
        .iter()
        .all(|item| item.filename() == "笔记😀.md"));
    f.recorded(&saved).await;
    let mut duplicate = serde_json::to_value(&desired).unwrap();
    duplicate["payload"][1] = duplicate["payload"][0].clone();
    assert!(serde_json::from_value::<TaskContentManifest>(duplicate).is_err());
    f.unchanged(&saved, 1).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_round_trips_empty_and_unicode_manifests_without_artifact_storage() {
    let f = Fixture::ready().await;
    let empty = f
        .store
        .create(id(), owner(), content("Discuss without code", vec![]))
        .await
        .unwrap();
    assert!(empty.task.input_refs.is_empty());
    assert!(empty.task.definition.is_none());
    assert_eq!(empty.task.status, TaskStatus::Draft);
    assert_eq!(empty.task.revision, rev(1));
    let requested = content("  Cafe\u{301} 📝  ", vec![artifact()]);
    let value = f
        .store
        .create(reference().task_id, owner(), requested.clone())
        .await
        .unwrap();
    assert_eq!(value.manifest, requested);
    assert_eq!(value.task.title, requested.title());
    assert_eq!(value.task.input_refs, requested.input_refs());
    let reopened = TaskContentStore::new(f.pool.clone(), scope());
    for item in [empty, value] {
        assert_eq!(
            reopened.get(item.task.reference, owner()).await.unwrap(),
            Some(item.clone())
        );
        f.recorded(&item).await;
    }
    let version: i32 = query("SELECT version FROM collaboration_task_schema")
        .fetch_one(&f.pool)
        .await
        .unwrap()
        .get("version");
    assert_eq!(version, 3);
    assert_eq!(f.count("collaboration_task_outbox").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_edits_title_filename_language_and_order_with_one_revision_each() {
    let f = Fixture::ready().await;
    let initial = f.create().await;
    let mut current = initial.clone();
    let mut next_pin = artifact();
    next_pin.revision_id = id();
    let title_only = content("Renamed task", vec![artifact()]);
    let filename_only = TaskContentManifest::new(
        "Renamed task",
        vec![TextPayloadBinding::new(artifact(), "renamed.txt", "markdown").unwrap()],
    )
    .unwrap();
    let language_only = TaskContentManifest::new(
        "Renamed task",
        vec![TextPayloadBinding::new(artifact(), "renamed.txt", "future.language").unwrap()],
    )
    .unwrap();
    for requested in [
        title_only,
        filename_only,
        language_only,
        content("Renamed task", vec![artifact(), next_pin.clone()]),
        content("Renamed task", vec![next_pin, artifact()]),
        content("Empty again", vec![]),
    ] {
        let after = f
            .store
            .replace(
                reference(),
                owner(),
                current.task.revision,
                requested.clone(),
            )
            .await
            .unwrap();
        assert_eq!(
            u64::from(after.task.revision),
            u64::from(current.task.revision) + 1
        );
        assert_eq!(after.task.reference, initial.task.reference);
        assert_eq!(after.task.owner, initial.task.owner);
        assert_eq!(after.task.created_at, initial.task.created_at);
        assert_eq!(after.task.status, TaskStatus::Draft);
        assert!(after.task.definition.is_none());
        assert_eq!(after.manifest, requested);
        assert_eq!(after.task.title, requested.title());
        assert_eq!(after.task.input_refs, requested.input_refs());
        f.recorded(&after).await;
        current = after;
    }
    assert_eq!(
        f.event_types().await,
        [
            "cyanrex.task.created",
            "cyanrex.task.content_updated",
            "cyanrex.task.content_updated",
            "cyanrex.task.content_updated",
            "cyanrex.task.content_updated",
            "cyanrex.task.content_updated",
            "cyanrex.task.content_updated"
        ]
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_owner_scope_and_duplicate_ids_never_widen_trusted_access() {
    let f = Fixture::ready().await;
    let original = f.create().await;
    assert_eq!(
        f.store
            .create(reference().task_id, owner(), manifest())
            .await,
        Err(TaskStoreError::Conflict)
    );
    let other = PrincipalRef {
        principal_id: id(),
        ..owner()
    };
    assert_eq!(f.store.get(reference(), other).await.unwrap(), None);
    assert_eq!(
        f.store
            .replace(reference(), other, rev(1), manifest())
            .await,
        Err(TaskStoreError::NotFound)
    );
    assert_eq!(
        f.store
            .transition(reference(), other, rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::NotFound)
    );
    let foreign = PrincipalRef {
        authority_id: id(),
        ..owner()
    };
    assert_eq!(
        f.store.create(id(), foreign, manifest()).await,
        Err(TaskStoreError::ScopeMismatch)
    );
    let wrong_scope = WorkspaceRef {
        workspace_id: id(),
        ..scope()
    };
    let wrong = TaskContentStore::new(f.pool.clone(), wrong_scope);
    assert_eq!(
        wrong
            .get(
                TaskRef {
                    workspace: wrong_scope,
                    ..reference()
                },
                owner()
            )
            .await,
        Err(TaskStoreError::ScopeMismatch)
    );
    let mut pin = artifact();
    pin.workspace = wrong_scope;
    let bad = content("Wrong scope", vec![pin]);
    assert_eq!(
        f.store.create(id(), owner(), bad.clone()).await,
        Err(TaskStoreError::ScopeMismatch)
    );
    assert_eq!(
        f.store.replace(reference(), owner(), rev(1), bad).await,
        Err(TaskStoreError::ScopeMismatch)
    );
    f.unchanged(&original, 1).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_stale_noop_and_non_draft_changes_have_no_side_effects() {
    let f = Fixture::ready().await;
    let initial = f.create().await;
    assert_eq!(
        f.store
            .replace(reference(), owner(), rev(1), initial.manifest.clone())
            .await,
        Err(TaskStoreError::InvalidInput)
    );
    let edited = f
        .store
        .replace(reference(), owner(), rev(1), content("Changed", vec![]))
        .await
        .unwrap();
    assert_eq!(
        f.store
            .replace(reference(), owner(), rev(1), manifest())
            .await,
        Err(TaskStoreError::StaleRevision)
    );
    assert_eq!(
        f.store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::StaleRevision)
    );
    let mut current = edited.clone();
    for status in [
        TaskStatus::Ready,
        TaskStatus::InProgress,
        TaskStatus::InReview,
        TaskStatus::Cancelled,
    ] {
        current = f
            .store
            .transition(reference(), owner(), current.task.revision, status)
            .await
            .unwrap();
        assert_eq!(current.manifest, edited.manifest);
        assert_eq!(current.task.title, edited.task.title);
        assert_eq!(current.task.input_refs, edited.task.input_refs);
        assert_eq!(
            f.store
                .replace(reference(), owner(), current.task.revision, manifest())
                .await,
            Err(TaskStoreError::InvalidTransition)
        );
        f.recorded(&current).await;
    }
    assert_eq!(
        f.store
            .transition(
                reference(),
                owner(),
                current.task.revision,
                TaskStatus::Ready
            )
            .await,
        Err(TaskStoreError::InvalidTransition)
    );
    f.unchanged(&current, 6).await;
    assert_eq!(
        f.event_types().await,
        [
            "cyanrex.task.created",
            "cyanrex.task.content_updated",
            "cyanrex.task.status_changed",
            "cyanrex.task.status_changed",
            "cyanrex.task.status_changed",
            "cyanrex.task.status_changed"
        ]
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_task_content_and_legacy_storage_reject_each_others_namespace_versions() {
    let f = Fixture::ready().await;
    let saved = f.create().await;
    let legacy = TaskStore::new(f.pool.clone(), scope());
    assert_eq!(
        legacy.get(reference(), owner()).await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    assert_eq!(
        legacy
            .create(id(), owner(), TaskDraft::manual("Legacy", vec![]).unwrap())
            .await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    assert_eq!(
        legacy
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    assert_eq!(
        legacy.install_empty_namespace().await,
        Err(TaskStoreError::NamespaceNotEmpty)
    );
    f.unchanged(&saved, 1).await;
    f.cleanup().await;
    let f = Fixture::new().await;
    let legacy = TaskStore::new(f.pool.clone(), scope());
    legacy.install_empty_namespace().await.unwrap();
    let saved = legacy
        .create(
            reference().task_id,
            owner(),
            TaskDraft::manual("Legacy", vec![]).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        f.store.get(reference(), owner()).await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    assert_eq!(
        f.store.create(id(), owner(), manifest()).await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    assert_eq!(
        f.store
            .replace(reference(), owner(), rev(1), manifest())
            .await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    assert_eq!(
        f.store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    assert_eq!(
        f.store.install_empty_namespace().await,
        Err(TaskStoreError::NamespaceNotEmpty)
    );
    assert_eq!(legacy.get(reference(), owner()).await.unwrap(), Some(saved));
    f.cleanup().await;
}
