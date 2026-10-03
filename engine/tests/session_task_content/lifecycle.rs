use super::*;

async fn denied(f: &Fixture, token: &str, task: &TaskContentSnapshot) {
    assert!(f.get(token, task.task.reference.task_id).await.is_err());
    assert!(f.request(token, id(), task.manifest.clone()).await.is_err());
    assert!(f
        .replace(
            token,
            task,
            manifest("Denied edit", task.task.input_refs.clone())
        )
        .await
        .is_err());
    assert!(f.transition(token, task, TaskStatus::Ready).await.is_err());
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_invalid_unbound_and_logged_out_sessions_never_expose_content() {
    let (f, task, _, _) = pair().await;
    denied(&f, "invalid", &task).await;
    denied(&f, &f.auth.target_token, &task).await;
    f.auth.bind_target().await;
    denied(&f, &f.auth.target_token, &task).await;
    f.auth.source.logout(&f.auth.learner_token).await.unwrap();
    denied(&f, &f.auth.learner_token, &task).await;
    f.unchanged(&task, 1).await;
    assert_eq!(f.files(), 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_suspended_membership_and_expired_sessions_fail_closed() {
    for expired in [false, true] {
        let (f, task, _, _) = pair().await;
        if expired {
            query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1")
                .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
        } else {
            let current = f.auth.current(f.auth.learner.binding.principal_id).await;
            let mut desired = current.policy;
            desired.membership.status = MembershipStatus::Suspended;
            f.auth
                .source
                .apply_session_policy_command(
                    &f.auth.manager_token,
                    &SessionPolicyCommand {
                        command_id: id(),
                        expected_revision: Some(current.revision),
                        desired,
                    },
                )
                .await
                .unwrap();
        }
        denied(&f, &f.auth.learner_token, &task).await;
        f.unchanged(&task, 1).await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_retired_and_recreated_accounts_cannot_inherit_saved_content() {
    let (f, task, old, _) = pair().await;
    let learner = &f.auth.learner;
    f.auth
        .store
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.auth.manager.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: learner.binding.username.clone(),
                account_id: learner.account_id,
                expected_principal: learner.binding.principal_id,
            },
        })
        .await
        .unwrap();
    denied(&f, &f.auth.learner_token, &task).await;
    let (token, principal) = f.replace_retired_learner().await;
    assert_ne!(principal, task.task.owner);
    assert_eq!(
        f.get(&token, task.task.reference.task_id).await.unwrap(),
        None
    );
    assert_eq!(
        f.replace(&token, &task, manifest("New owner", vec![]))
            .await,
        Err(SessionTaskError::Task(TaskStoreError::NotFound))
    );
    assert_eq!(
        f.request(&token, id(), manifest("Old bytes", vec![old.reference]))
            .await,
        Err(SessionTaskError::Artifact(ArtifactStoreError::NotFound))
    );
    let own = f.create_artifact(&token, b"Replacement account").await;
    let replacement = f
        .request(&token, id(), manifest("Own task", vec![own.reference]))
        .await
        .unwrap();
    assert_eq!(replacement.task.owner, principal);
    f.unchanged(&task, 2).await;
    f.cleanup().await;
}
