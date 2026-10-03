use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_missing_membership_and_revoked_sessions_never_expose_bytes() {
    let f = Fixture::ready().await;
    let input = f.create_artifact(&f.auth.learner_token, b"Private").await;
    let task = f.create(&f.auth.learner_token, vec![input.reference]).await;
    for bind_target in [false, true] {
        if bind_target {
            f.auth.bind_target().await;
        }
        assert!(f
            .get(&f.auth.target_token, task.reference.task_id)
            .await
            .is_err());
        assert!(f
            .request(&f.auth.target_token, id(), task.input_refs.clone())
            .await
            .is_err());
    }
    f.auth.source.logout(&f.auth.learner_token).await.unwrap();
    let invalid = SessionTaskError::Session(SessionCommandError::InvalidSession);
    assert_eq!(
        f.get(&f.auth.learner_token, task.reference.task_id).await,
        Err(invalid)
    );
    assert_eq!(
        f.request(&f.auth.learner_token, id(), task.input_refs.clone())
            .await,
        Err(invalid)
    );
    assert_eq!(
        f.transition(&f.auth.learner_token, &task, TaskStatus::Ready)
            .await,
        Err(invalid)
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.files(), 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_retired_and_recreated_accounts_cannot_inherit_evidence() {
    let f = Fixture::ready().await;
    let input = f
        .create_artifact(&f.auth.learner_token, b"Old account input")
        .await;
    let task = f
        .create(&f.auth.learner_token, vec![input.reference.clone()])
        .await;
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
        .get(&f.auth.learner_token, task.reference.task_id)
        .await
        .is_err());
    assert!(f
        .request(&f.auth.learner_token, id(), task.input_refs.clone())
        .await
        .is_err());
    assert!(f
        .transition(&f.auth.learner_token, &task, TaskStatus::Ready)
        .await
        .is_err());
    let (token, principal) = f.replace_retired_learner().await;
    assert_ne!(principal, task.owner);
    assert_eq!(f.get(&token, task.reference.task_id).await.unwrap(), None);
    assert_eq!(
        f.transition(&token, &task, TaskStatus::Ready).await,
        Err(SessionTaskError::Task(TaskStoreError::NotFound))
    );
    assert_eq!(
        f.request(&token, id(), task.input_refs.clone()).await,
        Err(SessionTaskError::Artifact(ArtifactStoreError::NotFound))
    );
    let own = f
        .create_artifact(&token, b"Replacement account input")
        .await;
    let replacement = f.create(&token, vec![own.reference]).await;
    assert_eq!(replacement.owner, principal);
    assert_eq!(
        f.tasks.get(task.reference, task.owner).await.unwrap(),
        Some(task)
    );
    assert_eq!(
        f.artifacts
            .read(&input.reference, input.owner)
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"Old account input"
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 2);
    f.cleanup().await;
}
