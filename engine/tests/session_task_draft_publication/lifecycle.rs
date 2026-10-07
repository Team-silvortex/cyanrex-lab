use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_each_step_reauthenticates_logout_and_membership_revocation() {
    for logout in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = prepare(&f, &[("a", "text", "confirmed"), ("b", "text", "denied")]);
        attempt.advance(&f.auth.learner_token).await.unwrap();
        let first = attempt.confirmed_artifacts().to_vec();
        let next = attempt.allocated_artifacts()[1].clone();
        let expected = if logout {
            f.auth.source.logout(&f.auth.learner_token).await.unwrap();
            SessionCommandError::InvalidSession
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
            SessionCommandError::Identity(IdentityStoreError::AccessDenied)
        };
        let result = attempt.advance(&f.auth.learner_token).await;
        let state = attempt.state().clone();
        let confirmed = attempt.confirmed_artifacts().to_vec();
        let retry = attempt.advance(&f.auth.learner_token).await;
        let after = counts(&f).await;
        f.cleanup().await;
        assert_eq!(
            result,
            Err(SessionDraftPublicationError::Artifact(
                SessionArtifactError::Session(expected)
            ))
        );
        assert_eq!(state, uncertain_artifact(1, next));
        assert_eq!(confirmed, first);
        assert_eq!(retry, Err(SessionDraftPublicationError::Stopped));
        assert_eq!(after, [1, 1, 1, 0, 0, 1]);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_final_task_wait_rechecks_database_session_expiry() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, &[("a", "text", "already committed")]);
    attempt.advance(&f.auth.learner_token).await.unwrap();
    let artifacts = attempt.confirmed_artifacts().to_vec();
    let target = attempt.task_ref();
    let mut holder = f.pause_task_event().await;
    let blocker = backend(&mut holder).await;
    query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
        .bind(source_fixture::token_hash(&f.auth.learner_token))
        .execute(&f.auth.base.pool).await.unwrap();
    let mut pending = Box::pin(attempt.advance(&f.auth.learner_token));
    let mut early = None;
    let waited = tokio::select! {
        result = &mut pending => { early = Some(result); false },
        waiter = blocked_by(&f, blocker) => waiter.is_some(),
    };
    // The request remains alive, already inside its write transaction. Expiry is observed in PG,
    // not inferred from a sleep or the application's clock.
    let expired = if waited {
        wait_expired(&f).await
    } else {
        false
    };
    holder.rollback().await.unwrap();
    let result = match early {
        Some(result) => result,
        None => pending.as_mut().await,
    };
    drop(pending);
    f.auth.base.drain().await;
    f.assert_pool_restored().await;
    let state = attempt.state().clone();
    let confirmed = attempt.confirmed_artifacts().to_vec();
    let no_task = attempt.confirmed_task().is_none();
    let retry = attempt.advance(&f.auth.learner_token).await;
    let after = counts(&f).await;
    f.cleanup().await;
    assert!(
        waited && expired,
        "must expire after reaching the exact task event barrier"
    );
    assert_eq!(
        result,
        Err(SessionDraftPublicationError::Task(
            SessionTaskError::Session(SessionCommandError::InvalidSession,)
        ))
    );
    assert_eq!(state, uncertain_task(target));
    assert_eq!(confirmed, artifacts);
    assert!(no_task);
    assert_eq!(retry, Err(SessionDraftPublicationError::Stopped));
    assert_eq!(after, [1, 1, 1, 0, 0, 1]);
}
