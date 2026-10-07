use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_state_and_scope_rejections_do_not_enter_source_sql() {
    let f = Fixture::ready().await;
    let attempt = unknown_task(&f, &[]).await;
    let checkpoint = checkpoint(&attempt);
    let ready = changed(&checkpoint, "/reported_state", json!("ready"));
    let complete = changed(&checkpoint, "/reported_state", json!("complete"));
    let foreign = WorkspaceRef {
        workspace_id: id(),
        ..scope()
    };
    let foreign_workspace = foreign_workspace(&f, foreign);
    let foreign_claim = changed(
        &checkpoint,
        "/task/workspace/workspace_id",
        json!(foreign.workspace_id),
    );
    let foreign_source = DurableAuthSource::new(f.auth.base.pool.clone(), id());
    let before = snapshot(&f).await;
    let mut holder = f.auth.base.pool.begin().await.unwrap();
    query("SELECT singleton FROM collaboration_auth_source_schema FOR UPDATE")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let results = tokio::time::timeout(Duration::from_secs(1), async {
        vec![
            inspect(&f, &f.auth.learner_token, &ready).await,
            inspect(&f, &f.auth.learner_token, &complete).await,
            f.auth
                .source
                .inspect_task_draft_checkpoint(
                    &f.auth.learner_token,
                    &foreign_workspace,
                    &checkpoint,
                )
                .await,
            inspect(&f, &f.auth.learner_token, &foreign_claim).await,
            foreign_source
                .inspect_task_draft_checkpoint(&f.auth.learner_token, &f.workspace, &checkpoint)
                .await,
        ]
    })
    .await;
    holder.rollback().await.unwrap();
    let after = snapshot(&f).await;
    drop(foreign_source);
    f.cleanup().await;
    assert_eq!(
        results.expect("state/scope rejects must not enter the locked auth source"),
        vec![
            Err(SessionDraftCheckpointInspectionError::NoUnconfirmedStep),
            Err(SessionDraftCheckpointInspectionError::NoUnconfirmedStep),
            Err(SessionDraftCheckpointInspectionError::ScopeMismatch),
            Err(SessionDraftCheckpointInspectionError::ScopeMismatch),
            Err(SessionDraftCheckpointInspectionError::ScopeMismatch),
        ]
    );
    assert_eq!(before, after);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_current_session_revocation_and_wait_expiry_discard_results() {
    for task in [false, true] {
        for mode in ["logout", "revoked", "expired", "wait_expiry"] {
            let f = Fixture::ready().await;
            let (_, checkpoint) = visible(&f, task).await;
            let mut holder = None;
            let expected = match mode {
                "logout" => {
                    f.auth.source.logout(&f.auth.learner_token).await.unwrap();
                    SessionCommandError::InvalidSession
                }
                "revoked" => {
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
                    SessionCommandError::Identity(IdentityStoreError::AccessDenied)
                }
                "wait_expiry" => {
                    holder = Some(schema_lock(&f, task).await);
                    query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
                    .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
                    SessionCommandError::InvalidSession
                }
                _ => {
                    query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1")
                    .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
                    SessionCommandError::InvalidSession
                }
            };
            let before = snapshot(&f).await;
            let mut waited = mode != "wait_expiry";
            let mut expired = waited;
            let result = if let Some(mut holder) = holder.take() {
                let blocker = backend(&mut holder).await;
                let mut pending = Box::pin(inspect(&f, &f.auth.learner_token, &checkpoint));
                let mut early = None;
                waited = tokio::select! {
                    result = &mut pending => { early = Some(result); false },
                    waiter = blocked_by(&f, blocker) => waiter.is_some(),
                };
                expired = if waited {
                    wait_expired(&f).await
                } else {
                    false
                };
                holder.rollback().await.unwrap();
                match early {
                    Some(result) => result,
                    None => pending.as_mut().await,
                }
            } else {
                inspect(&f, &f.auth.learner_token, &checkpoint).await
            };
            // End the optional transaction's fixture borrow before consuming the fixture.
            drop(holder);
            f.auth.base.drain().await;
            f.assert_pool_restored().await;
            let after = snapshot(&f).await;
            f.cleanup().await;
            assert!(
                waited && expired,
                "expiry must occur after the exact read blocker is observed"
            );
            assert_eq!(
                result,
                Err(if task {
                    SessionDraftCheckpointInspectionError::Task(SessionTaskError::Session(expected))
                } else {
                    SessionDraftCheckpointInspectionError::Artifact(SessionArtifactError::Session(
                        expected,
                    ))
                })
            );
            assert_eq!(before, after);
        }
    }
}
