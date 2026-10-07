use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_original_session_logout_revocation_and_expiry_still_deny() {
    for mode in ["logout", "revoked", "expired"] {
        let f = Fixture::ready().await;
        let attempt = unknown_artifact(&f, ITEMS, 0).await;
        publish_target(&f, &attempt, ITEMS, 0).await;
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
            _ => {
                query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1")
                    .bind(source_fixture::token_hash(&f.auth.learner_token))
                    .execute(&f.auth.base.pool).await.unwrap();
                SessionCommandError::InvalidSession
            }
        };
        let before = snapshot(&f).await;
        let result = attempt.observe_unconfirmed(&f.auth.learner_token).await;
        let after = snapshot(&f).await;
        f.cleanup().await;
        assert_eq!(
            result,
            Err(SessionDraftPublicationError::Artifact(
                SessionArtifactError::Session(expected)
            ))
        );
        assert_eq!(before, after);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_expiry_during_exact_read_lock_wait_discards_result() {
    let f = Fixture::ready().await;
    let attempt = unknown_artifact(&f, ITEMS, 0).await;
    publish_target(&f, &attempt, ITEMS, 0).await;
    let mut holder = lock_artifact_schema(&f).await;
    let blocker = backend(&mut holder).await;
    query("UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1")
        .bind(source_fixture::token_hash(&f.auth.learner_token))
        .execute(&f.auth.base.pool).await.unwrap();
    let before = snapshot(&f).await;
    let mut pending = Box::pin(attempt.observe_unconfirmed(&f.auth.learner_token));
    let mut early = None;
    let waited = tokio::select! {
        result = &mut pending => { early = Some(result); false },
        waiter = blocked_by(&f, blocker) => waiter.is_some(),
    };
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
    let after = snapshot(&f).await;
    let expected_state = uncertain_artifact(0, attempt.allocated_artifacts()[0].clone());
    let state = attempt.state().clone();
    f.cleanup().await;
    assert!(
        waited && expired,
        "must observe the exact read blocker before PG Session expiry"
    );
    assert_eq!(
        result,
        Err(SessionDraftPublicationError::Artifact(
            SessionArtifactError::Session(SessionCommandError::InvalidSession)
        ))
    );
    assert_eq!(before, after);
    assert_eq!(state, expected_state);
}
