use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_replaces_exact_inputs_without_rewriting_artifacts_or_reviews(
) {
    let (f, task, old, new) = pair().await;
    let review_schema = format!("replacement_reviews_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {review_schema}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let review_pool = pool_for(&review_schema).await;
    let reviews = ReviewStore::new(review_pool.clone(), scope());
    reviews.install_empty_namespace().await.unwrap();
    let review_workspace = SessionReviewWorkspace::new(
        &review_schema,
        scope(),
        f.artifact_workspace.clone(),
        catalog_fixture::definition(1).assessment_policy,
    )
    .unwrap();
    let review = f
        .auth
        .source
        .create_session_review(
            &f.auth.learner_token,
            &review_workspace,
            id(),
            vec![old.reference.clone()],
            vec![],
            HumanReviewEdit::new(
                HumanReviewVerdict::Approved,
                "Only the original was reviewed",
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let changed = replace(
        &f,
        &f.auth.learner_token,
        &task,
        vec![new.reference.clone()],
    )
    .await
    .unwrap();
    assert_eq!(changed.reference, task.reference);
    assert_eq!(changed.owner, task.owner);
    assert_eq!(changed.title, task.title);
    assert_eq!(changed.definition, task.definition);
    assert_eq!(changed.created_at, task.created_at);
    assert!(changed.updated_at >= task.updated_at);
    assert_eq!(changed.status, TaskStatus::Draft);
    assert_eq!(u64::from(changed.revision), 2);
    assert_eq!(changed.input_refs, vec![new.reference.clone()]);
    let loaded = f
        .get(&f.auth.learner_token, task.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.task, changed);
    assert_eq!(loaded.inputs[0].revision, new);
    assert_eq!(loaded.inputs[0].bytes, b"Revised input");
    assert_eq!(
        f.auth
            .source
            .read_session_review(&f.auth.learner_token, &review_workspace, review.reference,)
            .await
            .unwrap(),
        Some(review.clone())
    );
    assert_eq!(review.targets, vec![old.reference.clone()]);
    assert_eq!(
        event_types(&f, &changed).await,
        ["cyanrex.task.created", "cyanrex.task.inputs_replaced"]
    );
    let ready = f
        .transition(&f.auth.learner_token, &changed, TaskStatus::Ready)
        .await
        .unwrap();
    assert_eq!(ready.input_refs, changed.input_refs);
    assert_eq!(
        event_types(&f, &ready).await,
        [
            "cyanrex.task.created",
            "cyanrex.task.inputs_replaced",
            "cyanrex.task.status_changed"
        ]
    );
    published(&f, &old, &new).await;
    f.assert_pool_restored().await;
    review_pool.close().await;
    query(&format!("DROP SCHEMA {review_schema} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_catalog_zero_inputs_preserve_full_definition_and_adapter_policy(
) {
    let (f, manual, old, new) = pair().await;
    let definition = catalog_fixture::definition(1);
    let workspace = catalog_workspace(&f, vec![definition.clone()]);
    let empty = catalog_task(&f, &workspace, vec![]).await;
    let populated = catalog_replace(&f, &workspace, &empty, vec![new.reference.clone()])
        .await
        .unwrap();
    let cleared = catalog_replace(&f, &workspace, &populated, vec![])
        .await
        .unwrap();
    assert_eq!(cleared.definition, Some(definition.clone()));
    assert!(cleared.input_refs.is_empty());
    assert_eq!(cleared.status, TaskStatus::Draft);
    assert_eq!(u64::from(cleared.revision), 3);
    assert_eq!(
        catalog_replace(&f, &workspace, &cleared, vec![]).await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
    );
    assert_eq!(
        replace(&f, &f.auth.learner_token, &manual, vec![]).await,
        Err(SessionTaskError::UnsupportedTask)
    );
    assert_eq!(
        replace(
            &f,
            &f.auth.learner_token,
            &cleared,
            vec![old.reference.clone()]
        )
        .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    assert_eq!(
        catalog_replace(&f, &workspace, &manual, vec![new.reference.clone()]).await,
        Err(SessionTaskError::UnsupportedTask)
    );
    let no_inputs = f
        .auth
        .source
        .create_session_manual_task(
            &f.auth.learner_token,
            &f.task_workspace,
            id(),
            "Manual with no inputs",
        )
        .await
        .unwrap();
    assert_eq!(
        replace(
            &f,
            &f.auth.learner_token,
            &no_inputs,
            vec![new.reference.clone()]
        )
        .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    assert_eq!(
        f.auth
            .source
            .get_session_manual_task(
                &f.auth.learner_token,
                &f.task_workspace,
                populated.reference.task_id,
            )
            .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    for field in ["title", "summary", "policy", "schema"] {
        let mut changed = definition.clone();
        match field {
            "title" => changed.title.push_str(" changed"),
            "summary" => changed.summary.push_str(" changed"),
            "policy" => changed.assessment_policy.version = 2.try_into().unwrap(),
            "schema" => changed.evidence_schema.version = 2.try_into().unwrap(),
            _ => unreachable!(),
        }
        let mismatched = catalog_workspace(&f, vec![changed]);
        assert_eq!(
            catalog_replace(&f, &mismatched, &cleared, vec![new.reference.clone()]).await,
            Err(SessionTaskError::DefinitionMismatch),
            "{field}"
        );
    }
    let newer = catalog_workspace(&f, vec![catalog_fixture::definition(2)]);
    assert_eq!(
        catalog_replace(&f, &newer, &cleared, vec![new.reference.clone()]).await,
        Err(SessionTaskError::Task(TaskStoreError::UnknownDefinition))
    );
    unchanged(&f, &cleared, 5).await;
    let ready = f
        .auth
        .source
        .transition_session_catalog_task(
            &f.auth.learner_token,
            &workspace,
            cleared.reference.task_id,
            cleared.revision,
            TaskStatus::Ready,
        )
        .await
        .unwrap();
    assert_eq!(
        catalog_replace(&f, &workspace, &ready, vec![new.reference.clone()]).await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidTransition))
    );
    unchanged(&f, &ready, 6).await;
    published(&f, &old, &new).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_stale_replay_and_ordered_noop_are_not_successes() {
    let (f, task, old, new) = pair().await;
    assert_eq!(
        replace(&f, &f.auth.learner_token, &task, task.input_refs.clone()).await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
    );
    let two = replace(
        &f,
        &f.auth.learner_token,
        &task,
        vec![old.reference.clone(), new.reference.clone()],
    )
    .await
    .unwrap();
    assert_eq!(
        replace(&f, &f.auth.learner_token, &task, two.input_refs.clone()).await,
        Err(SessionTaskError::Task(TaskStoreError::StaleRevision))
    );
    assert_eq!(
        replace(&f, &f.auth.learner_token, &two, two.input_refs.clone()).await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
    );
    let reversed = replace(
        &f,
        &f.auth.learner_token,
        &two,
        vec![new.reference.clone(), old.reference.clone()],
    )
    .await
    .unwrap();
    assert_eq!(u64::from(reversed.revision), 3);
    let content = f
        .get(&f.auth.learner_token, task.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        content
            .inputs
            .iter()
            .map(|input| input.bytes.as_slice())
            .collect::<Vec<_>>(),
        [b"Revised input".as_slice(), b"Original input".as_slice()]
    );
    unchanged(&f, &reversed, 3).await;
    published(&f, &old, &new).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_invalid_inputs_and_other_owners_never_mutate_the_task() {
    let (f, task, old, new) = pair().await;
    let other = f
        .create_artifact(&f.auth.manager_token, b"Teacher owns this")
        .await;
    assert_eq!(
        replace(
            &f,
            &f.auth.manager_token,
            &task,
            vec![other.reference.clone()]
        )
        .await,
        Err(SessionTaskError::Task(TaskStoreError::NotFound))
    );
    for (refs, expected) in [
        (
            vec![new.reference.clone(); 2],
            SessionTaskError::Task(TaskStoreError::InvalidInput),
        ),
        (
            vec![
                new.reference.clone(),
                ArtifactRef {
                    sha256: "f".repeat(64).parse().unwrap(),
                    ..new.reference.clone()
                },
            ],
            SessionTaskError::Task(TaskStoreError::InvalidInput),
        ),
        (
            (0..33).map(|_| synthetic_ref()).collect(),
            SessionTaskError::Task(TaskStoreError::InvalidInput),
        ),
        (
            vec![other.reference],
            SessionTaskError::Artifact(ArtifactStoreError::NotFound),
        ),
        (
            vec![ArtifactRef {
                artifact_id: id(),
                ..new.reference.clone()
            }],
            SessionTaskError::Artifact(ArtifactStoreError::NotFound),
        ),
        (
            vec![ArtifactRef {
                revision_id: id(),
                ..new.reference.clone()
            }],
            SessionTaskError::Artifact(ArtifactStoreError::NotFound),
        ),
        (
            vec![ArtifactRef {
                sha256: "f".repeat(64).parse().unwrap(),
                ..old.reference.clone()
            }],
            SessionTaskError::Artifact(ArtifactStoreError::InvalidRecord),
        ),
    ] {
        assert_eq!(
            replace(&f, &f.auth.learner_token, &task, refs).await,
            Err(expected)
        );
        unchanged(&f, &task, 1).await;
    }
    for foreign in [
        WorkspaceRef {
            authority_id: id(),
            ..scope()
        },
        WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
    ] {
        assert!(matches!(
            replace(
                &f,
                &f.auth.learner_token,
                &task,
                vec![ArtifactRef {
                    workspace: foreign,
                    ..new.reference.clone()
                }]
            )
            .await,
            Err(SessionTaskError::Task(TaskStoreError::ScopeMismatch))
                | Err(SessionTaskError::Artifact(
                    ArtifactStoreError::ScopeMismatch
                ))
        ));
        unchanged(&f, &task, 1).await;
    }
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 3);
    assert_eq!(f.files(), 3);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_accepts_disjoint_32_by_32_union_without_losing_input_order()
{
    let f = Fixture::ready().await;
    let mut refs = Vec::new();
    for byte in 0..64u8 {
        refs.push(
            f.create_artifact(&f.auth.learner_token, &[byte])
                .await
                .reference,
        );
    }
    let task = f.create(&f.auth.learner_token, refs[..32].to_vec()).await;
    let next: Vec<_> = refs[32..].iter().rev().cloned().collect();
    let changed = replace(&f, &f.auth.learner_token, &task, next.clone())
        .await
        .unwrap();
    assert_eq!(changed.input_refs, next);
    let loaded = f
        .get(&f.auth.learner_token, task.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        loaded.inputs.iter().map(|v| v.bytes[0]).collect::<Vec<_>>(),
        (32..64u8).rev().collect::<Vec<_>>()
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 2);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 64);
    assert_eq!(f.files(), 64);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_storage_v2_rejects_legacy_v1_without_adopting_or_migrating()
{
    let (f, task, old, new) = pair().await;
    let version: i32 = query("SELECT version FROM collaboration_task_schema")
        .fetch_one(&f.task_pool)
        .await
        .unwrap()
        .get("version");
    assert_eq!(version, 2);
    let before: String = query("SELECT snapshot FROM collaboration_tasks")
        .fetch_one(&f.task_pool)
        .await
        .unwrap()
        .get("snapshot");
    let events: Vec<String> =
        query("SELECT snapshot FROM collaboration_task_outbox ORDER BY revision")
            .fetch_all(&f.task_pool)
            .await
            .unwrap()
            .iter()
            .map(|row| row.get("snapshot"))
            .collect();
    f.sql_task("UPDATE collaboration_task_schema SET version = 1")
        .await;
    assert_eq!(
        replace(
            &f,
            &f.auth.learner_token,
            &task,
            vec![new.reference.clone()]
        )
        .await,
        Err(SessionTaskError::Task(TaskStoreError::UnsupportedSchema))
    );
    assert_eq!(
        f.get(&f.auth.learner_token, task.reference.task_id).await,
        Err(SessionTaskError::Task(TaskStoreError::UnsupportedSchema))
    );
    let current: String = query("SELECT snapshot FROM collaboration_tasks")
        .fetch_one(&f.task_pool)
        .await
        .unwrap()
        .get("snapshot");
    let current_events: Vec<String> =
        query("SELECT snapshot FROM collaboration_task_outbox ORDER BY revision")
            .fetch_all(&f.task_pool)
            .await
            .unwrap()
            .iter()
            .map(|row| row.get("snapshot"))
            .collect();
    assert_eq!(current, before);
    assert_eq!(current_events, events);
    let version: i32 = query("SELECT version FROM collaboration_task_schema")
        .fetch_one(&f.task_pool)
        .await
        .unwrap()
        .get("version");
    assert_eq!(version, 1, "rejected calls must not migrate the namespace");
    published(&f, &old, &new).await;
    f.sql_task("UPDATE collaboration_task_schema SET version = 2")
        .await;
    f.cleanup().await;
}
