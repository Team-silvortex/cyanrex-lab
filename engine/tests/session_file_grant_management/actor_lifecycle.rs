use super::{fixture::*, publisher_fixture::validity, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_manager_rechecks_session_human_membership_audit_and_recreated_generation(
) {
    for mode in [
        "logout",
        "expired",
        "suspended",
        "retired",
        "agent",
        "identityaudit",
        "policyaudit",
    ] {
        let f = Fixture::ready().await;
        let old = issue(&f, 1, f.target()).await.unwrap();
        let expected = match mode {
            "logout" => {
                f.base
                    .auth
                    .source
                    .logout(&f.base.auth.learner_token)
                    .await
                    .unwrap();
                session_error(SessionCommandError::InvalidSession)
            }
            "expired" => {
                query("UPDATE sessions SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE token=$1").bind(source_fixture::token_hash(&f.base.auth.learner_token)).execute(&f.base.auth.base.pool).await.unwrap();
                session_error(SessionCommandError::InvalidSession)
            }
            "suspended" => {
                policy_status(
                    &f,
                    f.manager().principal().principal_id,
                    MembershipStatus::Suspended,
                )
                .await;
                identity_error(IdentityStoreError::AccessDenied)
            }
            "retired" => {
                retire(&f, &f.base.auth.learner).await;
                identity_error(IdentityStoreError::AccessDenied)
            }
            "agent" => {
                sql(&f,true,"UPDATE collaboration_principals SET kind='agent' WHERE display_name='source-learner'").await;
                identity_error(IdentityStoreError::InvalidRecord)
            }
            "identityaudit" => {
                sql(
                    &f,
                    true,
                    "UPDATE collaboration_identity_schema SET version=1",
                )
                .await;
                identity_error(IdentityStoreError::IdentityAuditNotEnabled)
            }
            _ => {
                sql(&f, true, "UPDATE collaboration_access_schema SET version=1").await;
                identity_error(IdentityStoreError::AuditNotEnabled)
            }
        };
        assert_eq!(issue(&f, 2, f.target()).await, Err(expected), "{mode}");
        assert_eq!(revoke(&f, 1, revision(1)).await, Err(expected), "{mode}");
        assert_eq!(
            direct_get(&f, 1, f.target()).await.unwrap(),
            Some(old.clone())
        );
        if mode == "retired" {
            let username = &f.base.auth.learner.binding.username;
            query("DELETE FROM users WHERE username=$1")
                .bind(username.as_str())
                .execute(&f.base.auth.base.pool)
                .await
                .unwrap();
            let created = f
                .base
                .auth
                .source
                .register(username, PASSWORD)
                .await
                .unwrap();
            let token = f
                .base
                .auth
                .source
                .login(username, PASSWORD, &otp(&created.bootstrap.secret))
                .await
                .unwrap()
                .token;
            let bound = f
                .base
                .auth
                .source
                .bind_session_account(
                    &f.base.auth.manager_token,
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
            f.base
                .auth
                .source
                .apply_session_policy_command(
                    &f.base.auth.manager_token,
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
            let new_actor = FileAccountRef::new(bound.principal.reference, bound.account_id);
            assert_ne!(new_actor, f.manager());
            assert_eq!(
                issue_as(&f, &token, &f.management, 2, f.target(), validity()).await,
                Err(SessionFilePublisherGrantError::PermissionDenied)
            );
            assert_eq!(
                revoke_as(&f, &token, &f.management, 1, revision(1)).await,
                Err(SessionFilePublisherGrantError::PermissionDenied)
            );
            let new_config = f
                .base
                .auth
                .source
                .file_grant_management_workspace(f.base.workspace.clone(), new_actor)
                .unwrap();
            let fresh = issue_as(&f, &token, &new_config, 2, f.target(), validity())
                .await
                .unwrap();
            assert_eq!(fresh.issuer(), new_actor);
            let revoked = revoke_as(&f, &token, &new_config, 1, revision(1))
                .await
                .unwrap();
            assert_eq!(revoked.issuer(), old.issuer());
        } else if matches!(
            mode,
            "agent" | "identityaudit" | "policyaudit" | "suspended"
        ) {
            match mode {"suspended"=>policy_status(&f,f.manager().principal().principal_id,MembershipStatus::Active).await,
                "agent"=>sql(&f,true,"UPDATE collaboration_principals SET kind='human' WHERE display_name='source-learner'").await,
                "identityaudit"=>sql(&f,true,"UPDATE collaboration_identity_schema SET version=2").await,_=>sql(&f,true,"UPDATE collaboration_access_schema SET version=2").await};
            issue(&f, 2, f.target()).await.unwrap();
            revoke(&f, 1, revision(1)).await.unwrap();
        }
        restored(&f).await;
        f.cleanup().await;
    }
}
