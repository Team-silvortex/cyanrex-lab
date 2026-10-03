use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_unbound_missing_membership_and_revoked_sessions_never_write() {
    let f = Fixture::ready().await;
    assert!(f
        .auth
        .source
        .create_session_artifact(
            &f.auth.target_token,
            &f.workspace,
            id(),
            id(),
            draft(b"Unbound")
        )
        .await
        .is_err());
    f.auth.bind_target().await;
    assert!(f
        .auth
        .source
        .create_session_artifact(
            &f.auth.target_token,
            &f.workspace,
            id(),
            id(),
            draft(b"No membership")
        )
        .await
        .is_err());
    assert_eq!(f.files(), 0);
    let first = f.create(&f.auth.learner_token, b"Before logout").await;
    f.auth.source.logout(&f.auth.learner_token).await.unwrap();
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&f.auth.learner_token, &f.workspace, &first.reference)
            .await,
        Err(SessionArtifactError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_eq!(
        f.auth
            .source
            .revise_session_artifact(
                &f.auth.learner_token,
                &f.workspace,
                &first.reference,
                id(),
                draft(b"Revoked")
            )
            .await,
        Err(SessionArtifactError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_eq!(
        f.auth
            .source
            .create_session_artifact(
                &f.auth.learner_token,
                &f.workspace,
                id(),
                id(),
                draft(b"Revoked")
            )
            .await,
        Err(SessionArtifactError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_eq!(f.files(), 1);
    assert_eq!(f.count("collaboration_artifact_outbox").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_artifacts_retired_and_recreated_accounts_cannot_inherit_content() {
    let f = Fixture::ready().await;
    let first = f
        .create(&f.auth.learner_token, b"Original account's document")
        .await;
    let old = &f.auth.learner;
    f.auth
        .store
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.auth.manager.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: old.binding.username.clone(),
                account_id: old.account_id,
                expected_principal: old.binding.principal_id,
            },
        })
        .await
        .unwrap();
    assert!(f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .is_some());
    assert!(f
        .auth
        .source
        .read_session_artifact(&f.auth.learner_token, &f.workspace, &first.reference)
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .revise_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            &first.reference,
            id(),
            draft(b"Retired")
        )
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .create_session_artifact(
            &f.auth.learner_token,
            &f.workspace,
            id(),
            id(),
            draft(b"Retired")
        )
        .await
        .is_err());
    assert_eq!(f.files(), 1);
    query("DELETE FROM users WHERE username = $1")
        .bind(old.binding.username.as_str())
        .execute(&f.auth.base.pool)
        .await
        .unwrap();
    let account = f
        .auth
        .source
        .register(&old.binding.username, PASSWORD)
        .await
        .unwrap();
    let token = f
        .auth
        .source
        .login(
            &old.binding.username,
            PASSWORD,
            &otp(&account.bootstrap.secret),
        )
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
                target: account.account,
            },
        )
        .await
        .unwrap()
        .entry
        .after;
    assert_ne!(bound.principal.reference, first.owner);
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
    assert_eq!(
        f.auth
            .source
            .read_session_artifact(&token, &f.workspace, &first.reference)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        f.auth
            .source
            .revise_session_artifact(
                &token,
                &f.workspace,
                &first.reference,
                id(),
                draft(b"Replacement account")
            )
            .await,
        Err(SessionArtifactError::Artifact(ArtifactStoreError::NotFound))
    );
    let new = f.create(&token, b"New account's document").await;
    assert_eq!(new.owner, bound.principal.reference);
    assert_eq!(
        f.artifacts
            .read(&first.reference, first.owner)
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"Original account's document"
    );
    f.cleanup().await;
}
