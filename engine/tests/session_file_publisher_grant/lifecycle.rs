use super::support::*;
use crate::{fixture::*, *};
use cyanrex_engine::services::collaboration_identity_store::{
    IdentityStoreError, LegacyIdentityAction, LegacyIdentityCommand,
};

async fn recreate(f: &Fixture) -> (String, StoredLegacyIdentity) {
    let username = &f.auth.learner.binding.username;
    query("DELETE FROM users WHERE username=$1")
        .bind(username.as_str())
        .execute(&f.auth.base.pool)
        .await
        .unwrap();
    let created = f.auth.source.register(username, PASSWORD).await.unwrap();
    let token = f
        .auth
        .source
        .login(username, PASSWORD, &otp(&created.bootstrap.secret))
        .await
        .unwrap()
        .token;
    let bound = f
        .auth
        .source
        .bind_session_account(
            &f.auth.manager_token,
            &SessionBindCommand {
                command_id: id(),
                workspace: scope(),
                target: created.account,
            },
        )
        .await
        .unwrap()
        .entry
        .after;
    f.auth
        .source
        .apply_session_policy_command(
            &f.auth.manager_token,
            &SessionPolicyCommand {
                command_id: id(),
                expected_revision: None,
                desired: LegacyAccessPolicy {
                    membership: Membership {
                        workspace: scope(),
                        principal_id: bound.binding.principal_id,
                        role_refs: vec![],
                        status: MembershipStatus::Active,
                    },
                    deployment_granted: false,
                },
            },
        )
        .await
        .unwrap();
    (token, bound)
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_publisher_grant_rechecks_current_membership_session_and_account_generation(
) {
    let f = Fixture::ready().await;
    assert_eq!(
        read(&f, &f.auth.target_token, 99).await,
        Err(SessionFilePublisherGrantError::Session(
            SessionCommandError::Identity(IdentityStoreError::AccessDenied)
        ))
    );
    let target = f.auth.bind_target().await;
    assert_eq!(
        read(&f, &f.auth.target_token, 99).await,
        Err(SessionFilePublisherGrantError::Session(
            SessionCommandError::Identity(IdentityStoreError::AccessDenied)
        ))
    );
    f.auth
        .source
        .apply_session_policy_command(
            &f.auth.manager_token,
            &SessionPolicyCommand {
                command_id: id(),
                expected_revision: None,
                desired: LegacyAccessPolicy {
                    membership: Membership {
                        workspace: scope(),
                        principal_id: target.binding.principal_id,
                        role_refs: vec![],
                        status: MembershipStatus::Active,
                    },
                    deployment_granted: false,
                },
            },
        )
        .await
        .unwrap();
    let target_account = FileAccountRef::new(target.principal.reference, target.account_id);
    let own = f.issue(99, target_account).await;
    assert_eq!(read(&f, &f.auth.target_token, 99).await.unwrap(), Some(own));
    f.cleanup().await;

    for mode in [
        "logout",
        "expired",
        "suspended",
        "retired",
        "archived",
        "rawrole",
        "agent",
        "identityaudit",
        "policyaudit",
    ] {
        let f = Fixture::ready().await;
        let old = f.issue(1, f.subject()).await;
        assert_eq!(
            read(&f, &f.auth.learner_token, 1).await.unwrap(),
            Some(old.clone())
        );
        let expected = match mode {
            "logout" => {
                f.auth.source.logout(&f.auth.learner_token).await.unwrap();
                SessionCommandError::InvalidSession
            }
            "expired" => {
                query("UPDATE sessions SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE token=$1")
                    .bind(source_fixture::token_hash(&f.auth.learner_token)).execute(&f.auth.base.pool).await.unwrap();
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
            "retired" => {
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
            "archived" => {
                query("UPDATE collaboration_workspaces SET status='archived'")
                    .execute(&f.auth.base.pool)
                    .await
                    .unwrap();
                SessionCommandError::Identity(IdentityStoreError::ScopeInactive)
            }
            "rawrole" => {
                assert_eq!(query("UPDATE collaboration_memberships SET role_refs=ARRAY['cyanrex.workspace.owner']::text[] WHERE principal_id=$1")
                    .bind(f.subject().principal().principal_id.as_uuid()).execute(&f.auth.base.pool).await.unwrap().rows_affected(), 1);
                SessionCommandError::Identity(IdentityStoreError::InvalidRecord)
            }
            "agent" => {
                assert_eq!(
                    query("UPDATE collaboration_principals SET kind='agent' WHERE principal_id=$1")
                        .bind(f.subject().principal().principal_id.as_uuid())
                        .execute(&f.auth.base.pool)
                        .await
                        .unwrap()
                        .rows_affected(),
                    1
                );
                SessionCommandError::Identity(IdentityStoreError::InvalidRecord)
            }
            "identityaudit" => {
                query("UPDATE collaboration_identity_schema SET version=1")
                    .execute(&f.auth.base.pool)
                    .await
                    .unwrap();
                SessionCommandError::Identity(IdentityStoreError::IdentityAuditNotEnabled)
            }
            _ => {
                query("UPDATE collaboration_access_schema SET version=1")
                    .execute(&f.auth.base.pool)
                    .await
                    .unwrap();
                SessionCommandError::Identity(IdentityStoreError::AuditNotEnabled)
            }
        };
        let before = f.snapshot().await;
        assert_eq!(
            read(&f, &f.auth.learner_token, 1).await,
            Err(SessionFilePublisherGrantError::Session(expected)),
            "{mode}"
        );
        assert_eq!(
            read(&f, &f.auth.learner_token, 99).await,
            Err(SessionFilePublisherGrantError::Session(expected)),
            "absence must not bypass {mode} authorization"
        );
        assert_eq!(f.snapshot().await, before);
        if mode == "retired" {
            let (token, bound) = recreate(&f).await;
            let account = FileAccountRef::new(bound.principal.reference, bound.account_id);
            assert_ne!(account, old.grant().subject());
            assert_eq!(read(&f, &token, 1).await.unwrap(), None);
            let fresh = f.issue(2, account).await;
            assert_eq!(read(&f, &token, 2).await.unwrap(), Some(fresh));
            assert_eq!(
                f.store.get(grant(1), old.grant().subject()).await.unwrap(),
                Some(old)
            );
        } else if mode == "suspended" {
            let current = f.auth.current(f.auth.learner.binding.principal_id).await;
            let mut desired = current.policy;
            desired.membership.status = MembershipStatus::Active;
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
            assert_eq!(read(&f, &f.auth.learner_token, 1).await.unwrap(), Some(old));
        } else if [
            "archived",
            "rawrole",
            "agent",
            "identityaudit",
            "policyaudit",
        ]
        .contains(&mode)
        {
            let restore = match mode {
                "archived" => "UPDATE collaboration_workspaces SET status='active'",
                "rawrole" => "UPDATE collaboration_memberships SET role_refs=ARRAY[]::text[] WHERE principal_id=(SELECT principal_id FROM collaboration_legacy_identities WHERE username='source-learner')",
                "agent" => "UPDATE collaboration_principals SET kind='human' WHERE display_name='source-learner'",
                "identityaudit" => "UPDATE collaboration_identity_schema SET version=2",
                _ => "UPDATE collaboration_access_schema SET version=2",
            };
            query(restore).execute(&f.auth.base.pool).await.unwrap();
            assert_eq!(read(&f, &f.auth.learner_token, 1).await.unwrap(), Some(old));
        }
        f.auth.base.drain().await;
        f.cleanup().await;
    }
}
