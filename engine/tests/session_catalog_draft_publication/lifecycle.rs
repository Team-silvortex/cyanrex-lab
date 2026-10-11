use super::sql_fixture::*;
use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_publication_each_artifact_and_task_step_reauthenticates_current_session_and_membership(
) {
    for final_task in [false, true] {
        for mode in ["logout", "expired", "suspended"] {
            let f = Fixture::ready().await;
            let items = if final_task {
                vec![("a", "text", "confirmed input")]
            } else {
                vec![
                    ("a", "text", "confirmed input"),
                    ("b", "text", "denied input"),
                ]
            };
            let mut attempt = prepare(&f, &values()[1], &items);
            attempt.advance(&f.auth.learner_token).await.unwrap();
            let first = attempt.confirmed_artifacts().to_vec();
            let expected = match mode {
                "logout" => {
                    f.auth.source.logout(&f.auth.learner_token).await.unwrap();
                    SessionCommandError::InvalidSession
                }
                "expired" => {
                    query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1")
                        .bind(source_fixture::token_hash(&f.auth.learner_token))
                        .execute(&f.auth.base.pool).await.unwrap();
                    SessionCommandError::InvalidSession
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
                    SessionCommandError::Identity(IdentityStoreError::AccessDenied)
                }
                _ => unreachable!(),
            };
            let before = observed(&f).await;
            let (error, state) = if final_task {
                (
                    SessionDraftPublicationError::Task(SessionTaskError::Session(expected)),
                    uncertain_task(attempt.task_ref()),
                )
            } else {
                (
                    SessionDraftPublicationError::Artifact(SessionArtifactError::Session(expected)),
                    uncertain_artifact(1, attempt.allocated_artifacts()[1].clone()),
                )
            };
            assert_eq!(
                attempt.advance(&f.auth.learner_token).await,
                Err(error),
                "{mode}, task={final_task}"
            );
            assert_eq!(attempt.state(), &state);
            assert_eq!(attempt.confirmed_artifacts(), first);
            assert!(attempt.confirmed_task().is_none());
            assert_eq!(
                observed(&f).await,
                before,
                "revocation cannot mutate business rows/files"
            );
            assert_eq!(
                attempt.advance(&f.auth.learner_token).await,
                Err(SessionDraftPublicationError::Stopped)
            );
            assert_eq!(observed(&f).await, before);
            f.assert_pool_restored().await;
            drop(attempt);
            f.cleanup().await;
        }
    }
}
