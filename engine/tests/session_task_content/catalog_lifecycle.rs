use super::catalog as c;
use super::*;
use std::time::Duration;

async fn denied(
    f: &Fixture,
    workspace: &SessionCatalogTaskContentWorkspace,
    token: &str,
    task: &TaskContentSnapshot,
) {
    assert!(f
        .auth
        .source
        .get_session_catalog_content_task(token, workspace, task.task.reference.task_id)
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .create_session_catalog_content_task(
            token,
            workspace,
            id(),
            &c::definition(1).reference,
            task.manifest.clone()
        )
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .replace_session_catalog_content_task(
            token,
            workspace,
            task.task.reference.task_id,
            task.task.revision,
            manifest("Denied edit", vec![])
        )
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .transition_session_catalog_content_task(
            token,
            workspace,
            task.task.reference.task_id,
            task.task.revision,
            TaskStatus::Ready
        )
        .await
        .is_err());
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_catalog_current_owner_lifecycle_and_expiry_after_event_wait_fail_closed(
) {
    for which in [
        "invalid",
        "unbound",
        "logout",
        "suspended",
        "expired",
        "retired",
    ] {
        let (f, workspace, task, old, _) = c::pair().await;
        let before = c::business_rows(&f).await;
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
        assert_eq!(
            f.auth
                .source
                .replace_session_catalog_content_task(
                    &f.auth.manager_token,
                    &workspace,
                    task.task.reference.task_id,
                    task.task.revision,
                    manifest("Not shared", vec![])
                )
                .await,
            Err(SessionTaskError::Task(TaskStoreError::NotFound))
        );
        assert_eq!(
            f.auth
                .source
                .transition_session_catalog_content_task(
                    &f.auth.manager_token,
                    &workspace,
                    task.task.reference.task_id,
                    task.task.revision,
                    TaskStatus::Ready,
                )
                .await,
            Err(SessionTaskError::Task(TaskStoreError::NotFound))
        );
        assert_eq!(
            f.auth
                .source
                .create_session_catalog_content_task(
                    &f.auth.manager_token,
                    &workspace,
                    id(),
                    &c::definition(1).reference,
                    manifest("Another owner's text", vec![old.reference.clone()]),
                )
                .await,
            Err(SessionTaskError::Artifact(ArtifactStoreError::NotFound))
        );
        match which {
            "invalid" => denied(&f, &workspace, "invalid", &task).await,
            "unbound" => {
                denied(&f, &workspace, &f.auth.target_token, &task).await;
                f.auth.bind_target().await;
                denied(&f, &workspace, &f.auth.target_token, &task).await;
            }
            "logout" => {
                f.auth.source.logout(&f.auth.learner_token).await.unwrap();
                denied(&f, &workspace, &f.auth.learner_token, &task).await;
            }
            "expired" => {
                query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1")
                    .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
                denied(&f, &workspace, &f.auth.learner_token, &task).await;
            }
            "suspended" => {
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
                denied(&f, &workspace, &f.auth.learner_token, &task).await;
            }
            "retired" => {
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
                denied(&f, &workspace, &f.auth.learner_token, &task).await;
                let (token, principal) = f.replace_retired_learner().await;
                assert_ne!(principal, task.task.owner);
                assert_eq!(
                    f.auth
                        .source
                        .get_session_catalog_content_task(
                            &token,
                            &workspace,
                            task.task.reference.task_id
                        )
                        .await
                        .unwrap(),
                    None
                );
                assert_eq!(
                    f.auth
                        .source
                        .replace_session_catalog_content_task(
                            &token,
                            &workspace,
                            task.task.reference.task_id,
                            task.task.revision,
                            manifest("No inheritance", vec![])
                        )
                        .await,
                    Err(SessionTaskError::Task(TaskStoreError::NotFound))
                );
                assert_eq!(
                    f.auth
                        .source
                        .create_session_catalog_content_task(
                            &token,
                            &workspace,
                            id(),
                            &c::definition(1).reference,
                            manifest("Old principal bytes", vec![old.reference.clone()])
                        )
                        .await,
                    Err(SessionTaskError::Artifact(ArtifactStoreError::NotFound))
                );
            }
            _ => unreachable!(),
        }
        assert_eq!(c::business_rows(&f).await, before, "{which}");
        assert_eq!(fs::read(f.blob(&old.reference)).unwrap(), b"Original input");
        assert_eq!(f.files(), 2);
        f.assert_pool_restored().await;
        drop(workspace);
        f.cleanup().await;
    }
    let (f, workspace, task, _, new) = c::pair().await;
    let before = c::business_rows(&f).await;
    let mut holder = f.pause_task_event().await;
    let blocker: i32 = query("SELECT pg_backend_pid() AS pid")
        .fetch_one(&mut *holder)
        .await
        .unwrap()
        .get("pid");
    query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
        .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
    let source = f.auth.source.clone();
    let token = f.auth.learner_token.clone();
    let copied = workspace.clone();
    let saved = task.task.clone();
    let pending = tokio::spawn(async move {
        source
            .replace_session_catalog_content_task(
                &token,
                &copied,
                saved.reference.task_id,
                saved.revision,
                manifest("Must not commit after expiry", vec![new.reference]),
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if query("SELECT pid FROM pg_stat_activity WHERE datname = current_database() AND $1 = ANY(pg_blocking_pids(pid)) LIMIT 1")
                .bind(blocker).fetch_optional(&f.auth.base.admin).await.unwrap().is_some() { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("catalog content edit did not reach its exact event blocker");
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let expired: bool = query(&format!("SELECT clock_timestamp() >= expires_at AS expired FROM {}.sessions WHERE token = $1", f.auth.base.schema))
                .bind(source_fixture::token_hash(&f.auth.learner_token)).fetch_one(&f.auth.base.admin).await.unwrap().get("expired");
            if expired { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("synthetic catalog Session did not expire");
    assert!(!pending.is_finished());
    holder.rollback().await.unwrap();
    assert_eq!(
        pending.await.unwrap(),
        Err(SessionTaskError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_eq!(c::business_rows(&f).await, before);
    assert_eq!(f.files(), 2);
    f.assert_pool_restored().await;
    drop(workspace);
    f.cleanup().await;
}
