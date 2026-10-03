use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_same_pin_metadata_changes_never_reinterpret_stored_work() {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    for field in [
        "title",
        "summary",
        "policy_name",
        "policy_version",
        "schema_name",
        "schema_version",
    ] {
        let mut changed = definition(1);
        match field {
            "title" => changed.title = "Different task title".into(),
            "summary" => changed.summary = "Different contract summary".into(),
            "policy_name" => {
                changed.assessment_policy.name = "sample.document.other".parse().unwrap()
            }
            "policy_version" => changed.assessment_policy.version = 2.try_into().unwrap(),
            "schema_name" => {
                changed.evidence_schema.name = "sample.document.other".parse().unwrap()
            }
            "schema_version" => changed.evidence_schema.version = 2.try_into().unwrap(),
            _ => unreachable!(),
        }
        let altered_catalog = catalog(vec![changed.clone()]);
        let task = f
            .tasks
            .create(
                id(),
                f.auth.learner.principal.reference,
                TaskDraft::from_catalog(
                    &altered_catalog,
                    &reference(),
                    "Stored under altered metadata",
                    vec![],
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            f.get(token, task.reference.task_id).await,
            Err(SessionTaskError::DefinitionMismatch),
            "{field}"
        );
        assert_eq!(
            f.transition(token, &task, TaskStatus::Ready).await,
            Err(SessionTaskError::DefinitionMismatch),
            "{field}"
        );
        assert_eq!(
            f.tasks
                .get(task.reference, task.owner)
                .await
                .unwrap()
                .unwrap()
                .definition,
            Some(changed)
        );
    }
    assert_eq!(f.count_task("collaboration_task_outbox").await, 6);
    assert_eq!(f.files(), 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_private_membership_never_grants_teacher_access_to_work() {
    let f = Fixture::ready().await;
    let current = f
        .auth
        .current(f.auth.learner.binding.principal_id)
        .await
        .policy;
    assert!(current.membership.role_refs.is_empty());
    assert!(!current.deployment_granted);
    let input = f
        .create_artifact(&f.auth.learner_token, b"Private text")
        .await;
    let task = f
        .create(&f.auth.learner_token, vec![input.reference.clone()])
        .await;
    fs::remove_file(f.blob(&input.reference)).unwrap();
    assert_eq!(
        f.get(&f.auth.manager_token, task.reference.task_id)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        f.transition(&f.auth.manager_token, &task, TaskStatus::Ready)
            .await,
        Err(SessionTaskError::Task(TaskStoreError::NotFound))
    );
    assert_eq!(
        f.request(&f.auth.manager_token, id(), task.input_refs.clone())
            .await,
        Err(SessionTaskError::Artifact(ArtifactStoreError::NotFound))
    );
    assert_eq!(
        f.get(&f.auth.learner_token, task.reference.task_id).await,
        Err(SessionTaskError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.files(), 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_invalid_exact_inputs_never_publish_partial_tasks() {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let own = f.create_artifact(token, b"Same bytes").await;
    let other = f
        .create_artifact(&f.auth.manager_token, b"Same bytes")
        .await;
    let missing = ArtifactRef {
        artifact_id: id(),
        ..own.reference.clone()
    };
    let missing_revision = ArtifactRef {
        revision_id: id(),
        ..own.reference.clone()
    };
    let wrong_digest = ArtifactRef {
        sha256: "f".repeat(64).parse().unwrap(),
        ..own.reference.clone()
    };
    let foreign_scope = ArtifactRef {
        workspace: WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        ..own.reference.clone()
    };
    for invalid in [
        other.reference,
        missing.clone(),
        missing_revision,
        wrong_digest,
        foreign_scope,
    ] {
        assert!(f.request(token, id(), vec![invalid]).await.is_err());
        assert_eq!(f.count_task("collaboration_tasks").await, 0);
        assert_eq!(f.count_task("collaboration_task_outbox").await, 0);
    }
    let task = f
        .tasks
        .create(
            id(),
            own.owner,
            TaskDraft::from_catalog(
                &catalog(vec![definition(1)]),
                &reference(),
                "Unresolved input",
                vec![own.reference, missing],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        f.get(token, task.reference.task_id).await,
        Err(SessionTaskError::Artifact(ArtifactStoreError::NotFound))
    );
    assert_eq!(
        f.transition(token, &task, TaskStatus::Cancelled).await,
        Err(SessionTaskError::Artifact(ArtifactStoreError::NotFound))
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 2);
    assert_eq!(f.files(), 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_revoked_retired_and_recreated_members_cannot_inherit_work()
{
    let f = Fixture::ready().await;
    let first = f.create(&f.auth.learner_token, vec![]).await;
    for bound in [false, true] {
        if bound {
            f.auth.bind_target().await;
        }
        assert!(f
            .get(&f.auth.target_token, first.reference.task_id)
            .await
            .is_err());
        assert!(f.request(&f.auth.target_token, id(), vec![]).await.is_err());
    }
    f.auth.source.logout(&f.auth.learner_token).await.unwrap();
    let invalid = SessionTaskError::Session(SessionCommandError::InvalidSession);
    assert_eq!(
        f.get(&f.auth.learner_token, first.reference.task_id).await,
        Err(invalid)
    );
    assert_eq!(
        f.request(&f.auth.learner_token, id(), vec![]).await,
        Err(invalid)
    );
    assert_eq!(
        f.transition(&f.auth.learner_token, &first, TaskStatus::Ready)
            .await,
        Err(invalid)
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    f.cleanup().await;

    let f = Fixture::ready().await;
    let first = f.create(&f.auth.learner_token, vec![]).await;
    let old = &f.auth.learner;
    f.auth
        .store
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.auth.manager.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: old.binding.username.clone(),
                account_id: old.account_id,
                expected_principal: old.binding.principal_id,
            },
        })
        .await
        .unwrap();
    assert!(f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .is_some());
    assert!(f
        .get(&f.auth.learner_token, first.reference.task_id)
        .await
        .is_err());
    assert!(f
        .request(&f.auth.learner_token, id(), vec![])
        .await
        .is_err());
    assert!(f
        .transition(&f.auth.learner_token, &first, TaskStatus::Ready)
        .await
        .is_err());
    let (token, principal) = f.replace_retired_learner().await;
    assert_ne!(principal, first.owner);
    assert_eq!(f.get(&token, first.reference.task_id).await.unwrap(), None);
    assert_eq!(
        f.transition(&token, &first, TaskStatus::Ready).await,
        Err(SessionTaskError::Task(TaskStoreError::NotFound))
    );
    let replacement = f.create(&token, vec![]).await;
    assert_eq!(replacement.owner, principal);
    assert_eq!(
        f.tasks.get(first.reference, first.owner).await.unwrap(),
        Some(first)
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 2);
    f.cleanup().await;
}
