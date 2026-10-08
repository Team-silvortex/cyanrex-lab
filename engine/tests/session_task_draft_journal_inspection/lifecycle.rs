use super::*;
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_inspection_requires_current_session_and_membership() {
    for mode in ["expired", "logout", "revoked", "malformed"] {
        let f = Fixture::ready().await;
        let attempt = f.journaled(ITEMS).await;
        let expected = if mode == "revoked" {
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
        } else {
            if mode == "logout" {
                f.auth.source.logout(&f.auth.learner_token).await.unwrap();
            }
            if mode == "expired" {
                query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1")
                    .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
            }
            SessionCommandError::InvalidSession
        };
        let before = snapshot(&f).await;
        let result = inspect(
            &f,
            if mode == "malformed" {
                "not-a-session"
            } else {
                &f.auth.learner_token
            },
            attempt.task_ref().task_id,
        )
        .await;
        let intact = before == snapshot(&f).await;
        f.cleanup().await;
        assert_eq!(
            result,
            Err(SessionDraftIntentError::Session(expected)),
            "{mode}"
        );
        assert!(intact);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_inspection_rechecks_expiry_after_journal_wait() {
    for present in [false, true] {
        let f = Fixture::ready().await;
        let attempt = f.journaled(ITEMS).await;
        let task_id = if present {
            attempt.task_ref().task_id
        } else {
            id()
        };
        let mut holder = f.schema_lock().await;
        let blocker = backend(&mut holder).await;
        query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
        .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
        let before = snapshot(&f).await;
        let mut pending = Box::pin(inspect(&f, &f.auth.learner_token, task_id));
        let mut early = None;
        let waited = tokio::select! {
            result = &mut pending => { early = Some(result); false },
            waiter = blocked_by(&f, blocker) => waiter.is_some(),
        };
        let expired = waited && wait_expired(&f).await;
        holder.rollback().await.unwrap();
        let result = match early {
            Some(result) => result,
            None => pending.as_mut().await,
        };
        drop(pending);
        f.auth.base.drain().await;
        f.assert_pool_restored().await;
        let intact = before == snapshot(&f).await;
        f.cleanup().await;
        assert!(waited && expired && intact);
        assert_eq!(
            result,
            Err(SessionDraftIntentError::Session(
                SessionCommandError::InvalidSession
            ))
        );
    }
}
