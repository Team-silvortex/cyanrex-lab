use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_non_draft_and_terminal_tasks_reject_replacement() {
    let (f, mut task, old, new) = pair().await;
    for status in [
        TaskStatus::Ready,
        TaskStatus::InProgress,
        TaskStatus::Blocked,
        TaskStatus::InProgress,
        TaskStatus::InReview,
        TaskStatus::Cancelled,
    ] {
        task = f
            .transition(&f.auth.learner_token, &task, status)
            .await
            .unwrap();
        assert_eq!(
            replace(
                &f,
                &f.auth.learner_token,
                &task,
                vec![new.reference.clone()]
            )
            .await,
            Err(SessionTaskError::Task(TaskStoreError::InvalidTransition)),
            "{status:?}"
        );
        unchanged(&f, &task, u64::from(task.revision) as i64).await;
        assert_eq!(task.input_refs, vec![old.reference.clone()]);
    }
    published(&f, &old, &new).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_invalid_expired_revoked_and_unbound_sessions_fail_closed() {
    for failure in ["invalid", "expired", "logout", "unbound"] {
        let (f, task, old, new) = pair().await;
        let token = match failure {
            "invalid" => "invalid",
            "unbound" => &f.auth.target_token,
            _ => &f.auth.learner_token,
        };
        if failure == "expired" {
            query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1")
                .bind(source_fixture::token_hash(token)).execute(&f.auth.base.pool).await.unwrap();
        } else if failure == "logout" {
            f.auth.source.logout(token).await.unwrap();
        }
        let result = replace(&f, token, &task, vec![new.reference.clone()]).await;
        if failure == "unbound" {
            assert!(matches!(result, Err(SessionTaskError::Session(_))));
        } else {
            assert_eq!(
                result,
                Err(SessionTaskError::Session(
                    SessionCommandError::InvalidSession
                )),
                "{failure}"
            );
        }
        unchanged(&f, &task, 1).await;
        published(&f, &old, &new).await;
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_retired_and_recreated_accounts_cannot_inherit_old_tasks() {
    let (f, task, old, new) = pair().await;
    let retired = &f.auth.learner;
    f.auth
        .store
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.auth.manager.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: retired.binding.username.clone(),
                account_id: retired.account_id,
                expected_principal: retired.binding.principal_id,
            },
        })
        .await
        .unwrap();
    assert!(matches!(
        replace(
            &f,
            &f.auth.learner_token,
            &task,
            vec![new.reference.clone()]
        )
        .await,
        Err(SessionTaskError::Session(_))
    ));
    let (token, principal) = f.replace_retired_learner().await;
    assert_ne!(principal, task.owner);
    assert_eq!(
        replace(&f, &token, &task, vec![new.reference.clone()]).await,
        Err(SessionTaskError::Task(TaskStoreError::NotFound))
    );
    unchanged(&f, &task, 1).await;
    published(&f, &old, &new).await;
    f.cleanup().await;
}
