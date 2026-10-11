use super::fixture::*;
use super::sql_fixture::*;
use super::*;
use cyanrex_engine::services::collaboration_identity_store::{
    IdentityStoreError, LegacyIdentityAction, LegacyIdentityCommand,
};

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_index_current_sessions_memberships_and_recreated_accounts_reauthorize(
) {
    for mode in ["logout", "expired", "suspended", "retired"] {
        let f = Fixture::ready().await;
        let workspace = index(&f);
        let task = f.create(vec![]).await;
        assert_eq!(
            list(&f, &workspace, &f.auth.learner_token, None, None)
                .await
                .unwrap()
                .entries,
            vec![entry(&task)]
        );
        assert_eq!(
            list(&f, &workspace, "invalid", None, None).await,
            Err(SessionTaskError::Session(
                SessionCommandError::InvalidSession
            ))
        );
        let inaccessible = Err(SessionTaskError::Session(SessionCommandError::Identity(
            IdentityStoreError::AccessDenied,
        )));
        assert_eq!(
            list(&f, &workspace, &f.auth.target_token, None, None).await,
            inaccessible,
            "a valid unbound account is not authorized by an empty namespace"
        );
        f.auth.bind_target().await;
        assert_eq!(
            list(&f, &workspace, &f.auth.target_token, None, None).await,
            inaccessible,
            "identity binding alone is not active private-work membership"
        );
        let expected = match mode {
            "logout" => {
                f.auth.source.logout(&f.auth.learner_token).await.unwrap();
                SessionCommandError::InvalidSession
            }
            "expired" => {
                assert_eq!(query("UPDATE sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE token = $1")
                    .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap().rows_affected(), 1);
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
            _ => {
                f.auth
                    .store
                    .apply_legacy_identity_command(&LegacyIdentityCommand {
                        command_id: id(),
                        actor: f.auth.manager.principal.reference,
                        workspace: scope(),
                        action: LegacyIdentityAction::Retire {
                            username: f.auth.learner.binding.username.clone(),
                            account_id: f.auth.learner.account_id,
                            expected_principal: f.auth.learner.binding.principal_id,
                        },
                    })
                    .await
                    .unwrap();
                SessionCommandError::Identity(IdentityStoreError::AccessDenied)
            }
        };
        assert_eq!(
            list(&f, &workspace, &f.auth.learner_token, None, None).await,
            Err(SessionTaskError::Session(expected)),
            "{mode}"
        );
        if mode == "retired" {
            let (token, principal) = f.replace_retired_learner().await;
            assert_ne!(principal, task.task.owner);
            let page = list(&f, &workspace, &token, None, None).await.unwrap();
            assert!(page.entries.is_empty());
            assert_eq!(page.next_after, None);
            let replacement = f
                .request(
                    &token,
                    id(),
                    manifest("Replacement account's own task", vec![]),
                )
                .await
                .unwrap();
            assert_eq!(replacement.task.owner, principal);
            assert_eq!(
                list(&f, &workspace, &token, None, None)
                    .await
                    .unwrap()
                    .entries,
                vec![entry(&replacement)]
            );
            assert_eq!(
                f.tasks
                    .get(task.task.reference, task.task.owner)
                    .await
                    .unwrap(),
                Some(task)
            );
        }
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}
