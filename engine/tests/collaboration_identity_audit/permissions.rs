use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_requires_current_manager_and_rejects_retired_actor_replay() {
    let (f, manager, backup, target) = ready().await;
    let manager_id = manager.binding.principal_id;
    for candidate in [
        actor(target.binding.principal_id),
        actor(id()),
        PrincipalRef {
            authority_id: id(),
            principal_id: manager_id,
        },
    ] {
        let mut cmd = retire(manager_id, &target);
        cmd.actor = candidate;
        assert_eq!(
            f.store.apply_legacy_identity_command(&cmd).await,
            Err(IdentityStoreError::AccessDenied)
        );
        assert_eq!(
            f.store
                .legacy_identity_audit(scope(), candidate, target.binding.principal_id, None, 10)
                .await,
            Err(IdentityStoreError::AccessDenied)
        );
    }
    let cmd = retire(manager_id, &manager);
    f.store.apply_legacy_identity_command(&cmd).await.unwrap();
    assert_eq!(
        f.store.apply_legacy_identity_command(&cmd).await,
        Err(IdentityStoreError::AccessDenied)
    );
    let policy_command = LegacyPolicyCommand {
        command_id: id(),
        actor: actor(manager_id),
        expected_revision: None,
        desired: policy(id(), true),
    };
    assert_eq!(
        f.store.apply_legacy_policy_command(&policy_command).await,
        Err(IdentityStoreError::AccessDenied)
    );
    assert_eq!(
        f.store
            .legacy_identity_audit(scope(), actor(manager_id), manager_id, None, 10)
            .await,
        Err(IdentityStoreError::AccessDenied)
    );
    assert_eq!(
        history(&f, backup.binding.principal_id, manager_id)
            .await
            .len(),
        2
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_retirement_and_grant_revocation_cannot_remove_all_managers() {
    let (f, manager, backup, _) = ready().await;
    let retirement = retire(manager.binding.principal_id, &manager);
    let revocation = LegacyPolicyCommand {
        command_id: id(),
        actor: actor(backup.binding.principal_id),
        expected_revision: Some(RevisionNumber::try_from(1).unwrap()),
        desired: policy(backup.binding.principal_id, false),
    };
    let peer = f.reopen().await;
    let (one, two) = tokio::join!(
        f.store.apply_legacy_identity_command(&retirement),
        peer.apply_legacy_policy_command(&revocation)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert_eq!(
        one.err().or(two.err()),
        Some(IdentityStoreError::LastAuthorityManager)
    );
    let mut remaining = 0;
    for identity in [&manager, &backup] {
        remaining += usize::from(
            f.store
                .preview_legacy_access(
                    scope(),
                    actor(identity.binding.principal_id),
                    LegacyAction::ManageDeployment,
                    &LegacyPolicyResource::Deployment {
                        authority_id: scope().authority_id,
                    },
                )
                .await
                .unwrap(),
        );
    }
    assert_eq!(remaining, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_retirement_invalidates_policy_but_does_not_invent_session_revocation() {
    let (f, manager, _, target) = ready().await;
    // These synthetic legacy tables are deliberately not the staging registry's authority.
    f.sql("CREATE TABLE users (username TEXT PRIMARY KEY)")
        .await;
    f.sql(
        "CREATE TABLE sessions (token TEXT PRIMARY KEY, username TEXT REFERENCES users(username))",
    )
    .await;
    f.sql("INSERT INTO users VALUES ('synthetic-student')")
        .await;
    f.sql("INSERT INTO sessions VALUES ('synthetic-not-a-real-token', 'synthetic-student')")
        .await;
    let principal = target.binding.principal_id;
    let policy_before = f
        .store
        .legacy_access(scope(), principal)
        .await
        .unwrap()
        .unwrap();
    f.store
        .apply_legacy_identity_command(&retire(manager.binding.principal_id, &target))
        .await
        .unwrap();
    assert_eq!(
        f.store.legacy_access(scope(), principal).await.unwrap(),
        Some(policy_before.clone())
    );
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ReadPrivateArtifact,
            &LegacyPolicyResource::PrivateArtifact {
                workspace: scope(),
                owner_id: principal
            }
        )
        .await
        .unwrap());
    let grant = LegacyPolicyCommand {
        command_id: id(),
        actor: actor(manager.binding.principal_id),
        expected_revision: Some(policy_before.revision),
        desired: policy(principal, true),
    };
    assert_eq!(
        f.store.apply_legacy_policy_command(&grant).await,
        Err(IdentityStoreError::IdentityRetired)
    );
    assert_eq!(
        query("SELECT count(*) AS count FROM sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .get::<i64, _>("count"),
        1
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_conflicting_bindings_and_archived_workspace_never_gain_identity() {
    let (f, manager, _, target) = ready().await;
    let manager = manager.binding.principal_id;
    let mut cmd = bind(manager);
    cmd.action = LegacyIdentityAction::Bind {
        username: username(),
        account_id: id(),
    };
    assert_eq!(
        f.store.apply_legacy_identity_command(&cmd).await,
        Err(IdentityStoreError::IdentityConflict)
    );
    cmd.action = LegacyIdentityAction::Bind {
        username: "other-student".parse().unwrap(),
        account_id: account(),
    };
    assert_eq!(
        f.store.apply_legacy_identity_command(&cmd).await,
        Err(IdentityStoreError::IdentityConflict)
    );
    f.sql("UPDATE collaboration_workspaces SET status = 'archived'")
        .await;
    assert_eq!(
        f.store.apply_legacy_identity_command(&bind(manager)).await,
        Err(IdentityStoreError::ScopeInactive)
    );
    f.store
        .apply_legacy_identity_command(&retire(manager, &target))
        .await
        .unwrap();
    assert_eq!(f.count("collaboration_principals").await, 3);
    assert_eq!(f.count("collaboration_identity_audit").await, 4);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_history_is_bounded_scoped_and_append_only() {
    let (f, manager, _, target) = ready().await;
    let manager = manager.binding.principal_id;
    let principal = target.binding.principal_id;
    f.store
        .apply_legacy_identity_command(&retire(manager, &target))
        .await
        .unwrap();
    let first = f
        .store
        .legacy_identity_audit(scope(), actor(manager), principal, None, 1)
        .await
        .unwrap();
    let next = f
        .store
        .legacy_identity_audit(
            scope(),
            actor(manager),
            principal,
            Some(first[0].sequence),
            1,
        )
        .await
        .unwrap();
    assert_eq!(next.len(), 1);
    assert!(next[0].sequence > first[0].sequence);
    assert!(f
        .store
        .legacy_identity_audit(
            scope(),
            actor(manager),
            principal,
            Some(next[0].sequence),
            1
        )
        .await
        .unwrap()
        .is_empty());
    assert!(history(&f, manager, id()).await.is_empty());
    for (cursor, limit) in [(None, 0), (None, 101), (Some(0), 1), (Some(u64::MAX), 1)] {
        assert_eq!(
            f.store
                .legacy_identity_audit(scope(), actor(manager), principal, cursor, limit)
                .await,
            Err(IdentityStoreError::InvalidRecord)
        );
    }
    for sql in [
        "UPDATE collaboration_identity_audit SET kind = kind",
        "DELETE FROM collaboration_identity_audit",
        "TRUNCATE collaboration_identity_audit",
    ] {
        assert!(query(sql).execute(&f.pool).await.is_err());
    }
    f.cleanup().await;
}
