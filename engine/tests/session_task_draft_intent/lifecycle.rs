use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_fresh_sessions_require_exact_owner_and_account_generation() {
    let f = Fixture::ready().await;
    let attempt = prepare(&f, ITEMS);
    let original = attempt
        .register_intent(&f.auth.learner_token, &f.intent_workspace)
        .await
        .unwrap();
    let expected = receipt(&original);
    let account = f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .unwrap()
        .account;
    f.auth.source.logout(&f.auth.learner_token).await.unwrap();
    // Explicit synthetic new device: tests fresh Session ownership, not a second OTP login.
    let fresh = f.auth.base.seed_session(&account).await.token;
    let before = f.state().await;
    let loaded = f.get(&fresh, attempt.task_ref().task_id).await;
    let loaded = loaded
        .as_ref()
        .map(|item| item.as_ref().map(receipt))
        .map_err(|error| *error);
    let manager = f
        .get(&f.auth.manager_token, attempt.task_ref().task_id)
        .await;
    let manager_absent = matches!(manager, Ok(None));
    let old_session = f
        .get(&f.auth.learner_token, attempt.task_ref().task_id)
        .await
        .err();
    let after_reads = f.state().await;
    // Hold owner fixed and alter only the account generation: an owner-only filter would leak it.
    query("UPDATE collaboration_draft_intents SET account_id = $1")
        .bind(f.auth.target.account_id.as_uuid())
        .execute(&f.journal_pool)
        .await
        .unwrap();
    let mismatched = f.get(&fresh, attempt.task_ref().task_id).await;
    let mismatched_absent = matches!(mismatched, Ok(None));
    let after_mismatch = f.count().await;
    let effects = counts(&f).await;
    f.cleanup().await;
    assert_eq!(loaded.unwrap(), Some(expected));
    assert!(manager_absent && mismatched_absent);
    assert_eq!(
        old_session,
        Some(SessionDraftIntentError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_eq!(before, after_reads);
    assert_eq!(after_mismatch, 1);
    assert_eq!(effects, [0; 6]);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_only_pristine_original_attempt_may_register() {
    let f = Fixture::ready().await;
    let pristine = prepare(&f, ITEMS);
    let mut prefix = prepare(&f, ITEMS);
    prefix.advance(&f.auth.learner_token).await.unwrap();
    let mut complete = prepare(&f, &[]);
    complete.advance(&f.auth.learner_token).await.unwrap();
    let mut unknown = prepare(&f, &[]);
    f.sql_task("UPDATE collaboration_task_schema SET version = 999")
        .await;
    assert!(unknown.advance(&f.auth.learner_token).await.is_err());
    f.sql_task("UPDATE collaboration_task_schema SET version = 3")
        .await;
    let account = f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .unwrap()
        .account;
    let fresh = f.auth.base.seed_session(&account).await.token;
    let before = counts(&f).await;
    let mut holder = f.auth.base.pool.begin().await.unwrap();
    query("SELECT singleton FROM collaboration_auth_source_schema FOR UPDATE")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let results = tokio::time::timeout(Duration::from_secs(1), async {
        vec![
            pristine
                .register_intent(&fresh, &f.intent_workspace)
                .await
                .err(),
            pristine
                .register_intent("malformed", &f.intent_workspace)
                .await
                .err(),
            prefix
                .register_intent(&f.auth.learner_token, &f.intent_workspace)
                .await
                .err(),
            complete
                .register_intent(&f.auth.learner_token, &f.intent_workspace)
                .await
                .err(),
            unknown
                .register_intent(&f.auth.learner_token, &f.intent_workspace)
                .await
                .err(),
        ]
    })
    .await;
    holder.rollback().await.unwrap();
    let after = counts(&f).await;
    let rows = f.count().await;
    f.cleanup().await;
    assert_eq!(
        results.expect("attempt admission must reject before locked source SQL"),
        vec![
            Some(SessionDraftIntentError::SessionChanged),
            Some(SessionDraftIntentError::SessionChanged),
            Some(SessionDraftIntentError::InvalidAttempt),
            Some(SessionDraftIntentError::InvalidAttempt),
            Some(SessionDraftIntentError::InvalidAttempt),
        ]
    );
    assert_eq!(rows, 0);
    assert_eq!(before, after);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_intent_current_session_and_wait_expiry_guard_register_and_read() {
    for read in [false, true] {
        for mode in ["logout", "revoked", "wait_expiry"] {
            let f = Fixture::ready().await;
            let attempt = prepare(&f, ITEMS);
            if read {
                attempt
                    .register_intent(&f.auth.learner_token, &f.intent_workspace)
                    .await
                    .unwrap();
            }
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
                _ => SessionCommandError::InvalidSession,
            };
            let mut holder = if mode == "wait_expiry" {
                Some(f.schema_lock().await)
            } else {
                None
            };
            if holder.is_some() {
                query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
                    .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
            }
            let before = f.state().await;
            let before_source = f.source_state().await;
            let command = async {
                if read {
                    f.get(&f.auth.learner_token, attempt.task_ref().task_id)
                        .await
                        .map(|_| ())
                } else {
                    attempt
                        .register_intent(&f.auth.learner_token, &f.intent_workspace)
                        .await
                        .map(|_| ())
                }
            };
            let mut pending = Box::pin(command);
            let mut waited = mode != "wait_expiry";
            let mut expired = waited;
            let result = if let Some(mut transaction) = holder.take() {
                let blocker = backend(&mut transaction).await;
                let mut early = None;
                waited = tokio::select! {
                    result = &mut pending => { early = Some(result); false },
                    waiter = blocked_by(&f, blocker) => waiter.is_some(),
                };
                expired = waited && wait_expired(&f).await;
                transaction.rollback().await.unwrap();
                match early {
                    Some(result) => result,
                    None => pending.as_mut().await,
                }
            } else {
                pending.as_mut().await
            };
            drop(pending);
            drop(holder);
            f.auth.base.drain().await;
            f.assert_pool_restored().await;
            let unchanged = before == f.state().await && before_source == f.source_state().await;
            let rows = f.count().await;
            let effects = counts(&f).await;
            f.cleanup().await;
            assert!(waited && expired && unchanged, "read={read}, mode={mode}");
            assert_eq!(result, Err(SessionDraftIntentError::Session(expected)));
            assert_eq!(rows, i64::from(read));
            assert_eq!(effects, [0; 6]);
        }
    }
}
