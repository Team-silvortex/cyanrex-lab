use super::*;
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_requires_current_session_and_membership() {
    for mode in ["logout", "expired", "revoked", "malformed"] {
        let f = Fixture::ready().await;
        let attempt = complete(&f, &[]).await;
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
        let result = observe(
            &f,
            if mode == "malformed" {
                "malformed"
            } else {
                &f.auth.learner_token
            },
            attempt.task_ref().task_id,
            0,
        )
        .await;
        let intact = before == snapshot(&f).await;
        f.cleanup().await;
        assert_eq!(
            result,
            Err(SessionDraftJournalObservationError::Intent(
                SessionDraftIntentError::Session(expected)
            ))
        );
        assert!(intact);
    }
}
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_resource_wait_expiry_and_cancellation_are_read_only() {
    for task in [false, true] {
        for cancel in [false, true] {
            let f = Fixture::ready().await;
            let attempt = complete(&f, ITEMS).await;
            let mut holder = resource_lock(&f, task).await;
            let blocker = backend(&mut holder).await;
            if !cancel {
                query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
                .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
            }
            let before = snapshot(&f).await;
            let mut pending = Box::pin(observe(
                &f,
                &f.auth.learner_token,
                attempt.task_ref().task_id,
                if task { ITEMS.len() } else { 0 },
            ));
            let mut early = None;
            let waited = tokio::select! { result = &mut pending => { early = Some(result); false }, waiter = blocked_by(&f, blocker) => waiter.is_some() };
            let expired = cancel || (waited && wait_expired(&f).await);
            let result = if cancel {
                drop(pending);
                holder.rollback().await.unwrap();
                None
            } else {
                holder.rollback().await.unwrap();
                let result = match early {
                    Some(value) => value,
                    None => pending.as_mut().await,
                };
                drop(pending);
                Some(result)
            };
            f.auth.base.drain().await;
            f.assert_pool_restored().await;
            let intact = before == snapshot(&f).await;
            f.cleanup().await;
            assert!(waited && expired && intact, "task={task}, cancel={cancel}");
            if let Some(result) = result {
                assert_eq!(
                    result,
                    Err(SessionDraftJournalObservationError::Intent(
                        SessionDraftIntentError::Session(SessionCommandError::InvalidSession)
                    ))
                );
            }
        }
    }
    // An owned intent with no recorded ordinal is still a pending answer: a wait must not
    // let NoRecordedStep bypass the final fresh Session check.
    let f = Fixture::ready().await;
    let attempt = f.journaled(&[]).await;
    let mut holder = f.schema_lock().await;
    let blocker = backend(&mut holder).await;
    query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
        .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
    let before = snapshot(&f).await;
    let mut pending = Box::pin(observe(
        &f,
        &f.auth.learner_token,
        attempt.task_ref().task_id,
        0,
    ));
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
        Err(SessionDraftJournalObservationError::Intent(
            SessionDraftIntentError::Session(SessionCommandError::InvalidSession)
        ))
    );
}
