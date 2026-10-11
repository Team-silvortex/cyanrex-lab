use super::{fixture::*, publication::Published, support::*, *};
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
async fn postgres_session_metadata_reauthorizes_logout_expiry_membership_and_recreated_accounts() {
    for mode in ["logout", "expired", "suspended", "retired"] {
        let f = Fixture::ready().await;
        let old = reserve(&f, &f.auth.learner_token, 1, &manifest(17, 1))
            .await
            .unwrap();
        let published = Published::empty(&f, &f.auth.learner_token, 2).await;
        let journal = published.rows(&f).await;
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
        let before = snapshot(&f).await;
        let read = get_upload(&f, &f.auth.learner_token, 1).await;
        let public = get_publication(&f, &f.auth.learner_token, 2).await;
        let write = reserve(&f, &f.auth.learner_token, 3, &manifest(17, 3)).await;
        assert_eq!(
            read,
            Err(SessionFileMetadataError::Session(expected)),
            "{mode}"
        );
        assert_eq!(
            public,
            Err(SessionFileMetadataError::Session(expected)),
            "{mode}"
        );
        assert_eq!(
            write,
            Err(SessionFileMetadataError::Session(expected)),
            "{mode}"
        );
        assert_eq!(snapshot(&f).await, before);
        if mode == "retired" {
            let (token, bound) = recreate(&f).await;
            assert_ne!(bound.account_id, old.account_id());
            assert_ne!(bound.principal.reference, old.owner());
            assert_eq!(get_upload(&f, &token, 1).await.unwrap(), None);
            assert_eq!(get_publication(&f, &token, 2).await.unwrap(), None);
            let fresh = reserve(&f, &token, 4, &manifest(17, 4)).await.unwrap();
            assert_eq!(fresh.owner(), bound.principal.reference);
            assert_eq!(fresh.account_id(), bound.account_id);
            assert_eq!(get_upload(&f, &token, 4).await.unwrap(), Some(fresh));
            assert_eq!(
                f.uploads
                    .get(upload(1), old.owner(), old.account_id())
                    .await
                    .unwrap(),
                Some(old)
            );
        }
        assert_eq!(published.rows(&f).await, journal);
        f.pool_restored().await;
        published.cleanup(&f).await;
        f.cleanup().await;
    }
}
