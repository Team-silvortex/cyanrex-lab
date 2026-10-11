use super::fixture::*;
use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_observation_original_session_reauthenticates_each_read_and_lock_expiry_discards_results(
) {
    for task in [false, true] {
        for mode in ["logout", "expired", "suspended", "lock-expired"] {
            let f = Fixture::ready().await;
            let mut attempt = if task {
                let attempt = unknown_task(&f, 0, ITEMS).await;
                create_target(&f, &attempt, &f.auth.learner_token, 0, ITEMS).await;
                attempt
            } else {
                let attempt = unknown_artifact(&f, 0, ITEMS, 0).await;
                publish_target(&f, &attempt, &f.auth.learner_token, ITEMS, 0).await;
                attempt
            };
            let expected = if mode == "suspended" {
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
                } else {
                    query(if mode == "expired" { "UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1" } else { "UPDATE sessions SET expires_at = clock_timestamp() + INTERVAL '800 milliseconds' WHERE token = $1" })
                        .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
                }
                SessionCommandError::InvalidSession
            };
            let result = if mode == "lock-expired" {
                let before = snapshot(&f).await;
                let saved = progress(&attempt);
                let mut holder = lock_resource(&f, task).await;
                let blocker = backend(&mut holder).await;
                let mut pending = Box::pin(attempt.observe_unconfirmed(&f.auth.learner_token));
                let mut early = None;
                let waited = tokio::select! { result = &mut pending => { early = Some(result); false }, waiter = blocked_by(&f,blocker) => waiter.is_some() };
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
                assert!(
                    waited && expired,
                    "the read must reach its exact lock before PG expiry"
                );
                assert_eq!(snapshot(&f).await, before);
                assert_eq!(progress(&attempt), saved);
                result
            } else {
                observe_preserved(&f, &attempt).await
            };
            let error = if task {
                SessionDraftPublicationError::Task(SessionTaskError::Session(expected))
            } else {
                SessionDraftPublicationError::Artifact(SessionArtifactError::Session(expected))
            };
            assert_eq!(result, Err(error));
            stopped(&f, &mut attempt).await;
            drop(attempt);
            f.cleanup().await;
        }
    }
}
