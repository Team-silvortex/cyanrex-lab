use super::{fixture::*, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_file_publication_rechecks_logout_expiry_human_membership_audit_and_account_recreation(
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
        let f = Fixture::ready(17).await;
        f.grants.issue(1, f.actor()).await;
        let mut a = prepare(&f, 1).await;
        let expected = match mode {
            "logout" => {
                f.grants
                    .auth
                    .source
                    .logout(&f.grants.auth.learner_token)
                    .await
                    .unwrap();
                session_error(SessionCommandError::InvalidSession)
            }
            "expired" => {
                sql(&f,&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE token='{}'",f.grants.auth.base.schema,source_fixture::token_hash(&f.grants.auth.learner_token))).await;
                session_error(SessionCommandError::InvalidSession)
            }
            "suspended" => {
                policy_status(&f, MembershipStatus::Suspended).await;
                identity_error(IdentityStoreError::AccessDenied)
            }
            "retired" => {
                f.grants
                    .auth
                    .store
                    .apply_legacy_identity_command(&LegacyIdentityCommand {
                        command_id: id(),
                        actor: f.grants.issuer().principal(),
                        workspace: scope(),
                        action: LegacyIdentityAction::Retire {
                            username: f.grants.auth.learner.binding.username.clone(),
                            account_id: f.actor().account_id(),
                            expected_principal: f.actor().principal().principal_id,
                        },
                    })
                    .await
                    .unwrap();
                identity_error(IdentityStoreError::AccessDenied)
            }
            "agent" => {
                sql(&f,&format!("UPDATE {}.collaboration_principals SET kind='agent' WHERE principal_id='{}'",f.grants.auth.base.schema,f.actor().principal().principal_id)).await;
                identity_error(IdentityStoreError::InvalidRecord)
            }
            "identityaudit" => {
                sql(
                    &f,
                    &format!(
                        "UPDATE {}.collaboration_identity_schema SET version=1",
                        f.grants.auth.base.schema
                    ),
                )
                .await;
                identity_error(IdentityStoreError::IdentityAuditNotEnabled)
            }
            _ => {
                sql(
                    &f,
                    &format!(
                        "UPDATE {}.collaboration_access_schema SET version=1",
                        f.grants.auth.base.schema
                    ),
                )
                .await;
                identity_error(IdentityStoreError::AuditNotEnabled)
            }
        };
        let before = f.snapshot().await;
        let result = publish(&f, &mut a).await;
        let state = a.state();
        let after = f.snapshot().await;
        let healthy = if mode == "retired" {
            let username = f.grants.auth.learner.binding.username.clone();
            sql(
                &f,
                &format!(
                    "DELETE FROM {}.users WHERE username='{}'",
                    f.grants.auth.base.schema, username
                ),
            )
            .await;
            let created = f
                .grants
                .auth
                .source
                .register(&username, PASSWORD)
                .await
                .unwrap();
            let token = f
                .grants
                .auth
                .source
                .login(&username, PASSWORD, &otp(&created.bootstrap.secret))
                .await
                .unwrap()
                .token;
            let bound = f
                .grants
                .auth
                .source
                .bind_session_account(
                    &f.grants.auth.manager_token,
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
            f.grants
                .auth
                .source
                .apply_session_policy_command(
                    &f.grants.auth.manager_token,
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
            assert_ne!(new_actor, f.actor());
            let mut old = prepare(&f, 1).await;
            let denied = publish_as(&f, &token, &f.workspace, 1, &mut old).await;
            assert_eq!(
                denied.err(),
                Some(SessionFilePublishingError::PermissionDenied)
            );
            f.stage_additional(2, 17, new_actor).await;
            f.grants.issue(2, new_actor).await;
            let mut fresh = f
                .publisher
                .prepare(upload(2), new_actor.principal(), new_actor.account_id())
                .await
                .unwrap();
            publish_as(&f, &token, &f.workspace, 2, &mut fresh).await
        } else {
            match mode {"logout"=>{},"expired"=>sql(&f,&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+INTERVAL '12 hours' WHERE token='{}'",f.grants.auth.base.schema,source_fixture::token_hash(&f.grants.auth.learner_token))).await,
            "suspended"=>policy_status(&f,MembershipStatus::Active).await,"agent"=>sql(&f,&format!("UPDATE {}.collaboration_principals SET kind='human' WHERE principal_id='{}'",f.grants.auth.base.schema,f.actor().principal().principal_id)).await,
            "identityaudit"=>sql(&f,&format!("UPDATE {}.collaboration_identity_schema SET version=2",f.grants.auth.base.schema)).await,_=>sql(&f,&format!("UPDATE {}.collaboration_access_schema SET version=2",f.grants.auth.base.schema)).await};
            let token = if mode == "logout" {
                f.grants
                    .auth
                    .base
                    .seed_session(&DurableAccountRef {
                        authority_id: authority(),
                        username: f.grants.auth.learner.binding.username.clone(),
                        account_id: f.actor().account_id(),
                    })
                    .await
                    .token
            } else {
                f.grants.auth.learner_token.clone()
            };
            let mut fresh = prepare(&f, 1).await;
            publish_as(&f, &token, &f.workspace, 1, &mut fresh).await
        };
        f.cleanup().await;
        assert_eq!(result.err(), Some(expected), "{mode}");
        assert_eq!(state, FileUploadPublicationState::Unknown);
        assert_eq!(after, before);
        assert!(healthy.is_ok(), "restored/new generation {mode}");
    }
}
