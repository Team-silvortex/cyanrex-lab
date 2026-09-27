use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_authority_scopes_isolate_bootstrap_commands_and_history() {
    let (f, manager, _, target) = seeded().await;
    let other = WorkspaceRef {
        authority_id: id(),
        workspace_id: id(),
    };
    f.store.provision_legacy_workspace(other).await.unwrap();
    assert_eq!(
        f.store.upgrade_policy_audit_schema().await,
        Err(IdentityStoreError::AuditBootstrapRequired)
    );
    let foreign = f
        .store
        .bind_legacy_account(other, &"manager-one".parse().unwrap(), id())
        .await
        .unwrap()
        .binding
        .principal_id;
    let foreign_actor = PrincipalRef {
        authority_id: other.authority_id,
        principal_id: foreign,
    };
    let mut foreign_policy = policy(foreign, true);
    foreign_policy.membership.workspace = other;
    let original = f
        .store
        .replace_legacy_access(None, &foreign_policy)
        .await
        .unwrap();
    f.store.upgrade_policy_audit_schema().await.unwrap();
    let local_command = command(manager, target);
    let local = f
        .store
        .apply_legacy_policy_command(&local_command)
        .await
        .unwrap();
    let foreign_command = LegacyPolicyCommand {
        command_id: local_command.command_id,
        actor: foreign_actor,
        expected_revision: Some(original.revision),
        desired: foreign_policy,
    };
    let remote = f
        .store
        .apply_legacy_policy_command(&foreign_command)
        .await
        .unwrap();
    assert!(!remote.replayed);
    assert_eq!(remote.entry.kind, PolicyAuditKind::Unchanged);
    assert_eq!(remote.entry.workspace, other);
    assert_eq!(local.entry.workspace, scope());
    let forged = LegacyPolicyCommand {
        actor: actor(manager),
        ..foreign_command.clone()
    };
    assert_eq!(
        f.store.apply_legacy_policy_command(&forged).await,
        Err(IdentityStoreError::AccessDenied)
    );
    assert_eq!(
        f.store
            .legacy_policy_audit(other, actor(manager), foreign, None, 10)
            .await,
        Err(IdentityStoreError::AccessDenied)
    );
    assert!(f
        .store
        .legacy_policy_audit(other, foreign_actor, target, None, 10)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        f.store
            .legacy_policy_audit(other, foreign_actor, foreign, None, 10)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(f.count("collaboration_policy_audit").await, 6);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_requires_current_instance_authority_for_commands_and_history() {
    let (f, manager, backup, target) = ready().await;
    let mut candidates = vec![
        actor(target),
        actor(id()),
        PrincipalRef {
            authority_id: id(),
            principal_id: manager,
        },
    ];
    f.sql(&format!(
        "UPDATE collaboration_principals SET status = 'disabled' WHERE principal_id = '{}'",
        backup
    ))
    .await;
    candidates.push(actor(backup));
    for candidate in candidates {
        let mut cmd = command(manager, target);
        cmd.actor = candidate;
        assert_eq!(
            f.store.apply_legacy_policy_command(&cmd).await,
            Err(IdentityStoreError::AccessDenied)
        );
        assert_eq!(
            f.store
                .legacy_policy_audit(scope(), candidate, target, None, 10)
                .await,
            Err(IdentityStoreError::AccessDenied)
        );
    }
    for role in ["cyanrex.workspace.owner", "cyanrex.teaching.teacher"] {
        let mut unprivileged = command(manager, target);
        unprivileged.expected_revision = Some(current(&f, target).await.revision);
        unprivileged.desired.deployment_granted = false;
        unprivileged.desired.membership.role_refs = vec![role.parse().unwrap()];
        f.store
            .apply_legacy_policy_command(&unprivileged)
            .await
            .unwrap();
        assert_eq!(
            f.store
                .apply_legacy_policy_command(&command(target, manager))
                .await,
            Err(IdentityStoreError::AccessDenied)
        );
        assert_eq!(
            f.store
                .legacy_policy_audit(scope(), actor(target), target, None, 10)
                .await,
            Err(IdentityStoreError::AccessDenied)
        );
    }
    let mut wrong_scope = command(manager, target);
    wrong_scope.desired.membership.workspace.workspace_id = id();
    assert_eq!(
        f.store.apply_legacy_policy_command(&wrong_scope).await,
        Err(IdentityStoreError::ScopeConflict)
    );
    assert_eq!(f.count("collaboration_policy_audit").await, 5);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_preserves_last_manager_and_rechecks_authority_on_replay() {
    let (f, manager, backup, _) = ready().await;
    let mut revoke = command(manager, manager);
    revoke.desired = policy(manager, false);
    f.store.apply_legacy_policy_command(&revoke).await.unwrap();
    assert_eq!(
        f.store.apply_legacy_policy_command(&revoke).await,
        Err(IdentityStoreError::AccessDenied)
    );
    assert_eq!(
        f.store
            .legacy_policy_audit(scope(), actor(manager), manager, None, 10)
            .await,
        Err(IdentityStoreError::AccessDenied)
    );
    let mut last = command(backup, backup);
    last.desired = policy(backup, false);
    assert_eq!(
        f.store.apply_legacy_policy_command(&last).await,
        Err(IdentityStoreError::LastAuthorityManager)
    );
    assert!(current(&f, backup).await.policy.deployment_granted);
    assert_eq!(f.count("collaboration_policy_audit").await, 4);
    last.desired = policy(backup, true);
    last.desired.membership.status = MembershipStatus::Suspended;
    f.store.apply_legacy_policy_command(&last).await.unwrap();
    assert_eq!(history(&f, backup, backup).await.len(), 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_history_is_bounded_keyset_scoped_and_append_only() {
    let (f, manager, _, target) = ready().await;
    f.store
        .apply_legacy_policy_command(&command(manager, target))
        .await
        .unwrap();
    let first = f
        .store
        .legacy_policy_audit(scope(), actor(manager), target, None, 1)
        .await
        .unwrap();
    let next = f
        .store
        .legacy_policy_audit(scope(), actor(manager), target, Some(first[0].sequence), 1)
        .await
        .unwrap();
    assert_eq!(next.len(), 1);
    assert!(next[0].sequence > first[0].sequence);
    assert!(f
        .store
        .legacy_policy_audit(scope(), actor(manager), target, Some(next[0].sequence), 1)
        .await
        .unwrap()
        .is_empty());
    assert!(history(&f, manager, id()).await.is_empty());
    for (cursor, limit) in [(None, 0), (None, 101), (Some(0), 1), (Some(u64::MAX), 1)] {
        assert_eq!(
            f.store
                .legacy_policy_audit(scope(), actor(manager), target, cursor, limit)
                .await,
            Err(IdentityStoreError::InvalidRecord)
        );
    }
    for sql in [
        "UPDATE collaboration_policy_audit SET kind = kind",
        "DELETE FROM collaboration_policy_audit",
        "TRUNCATE collaboration_policy_audit",
    ] {
        assert!(query(sql).execute(&f.pool).await.is_err());
    }
    assert_eq!(f.count("collaboration_policy_audit").await, 4);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_retired_incarnations_never_inherit_commands_or_authority() {
    let (f, manager, backup, target) = ready().await;
    let cmd = command(manager, target);
    f.store.apply_legacy_policy_command(&cmd).await.unwrap();
    f.store
        .retire_legacy_account(scope(), &username(), account(), target)
        .await
        .unwrap();
    let replacement = f
        .store
        .bind_legacy_account(scope(), &username(), id())
        .await
        .unwrap()
        .binding
        .principal_id;
    assert_ne!(target, replacement);
    assert_eq!(
        f.store.legacy_access(scope(), replacement).await.unwrap(),
        None
    );
    let mut denied = command(target, backup);
    assert_eq!(
        f.store.apply_legacy_policy_command(&denied).await,
        Err(IdentityStoreError::AccessDenied)
    );
    denied.actor = actor(replacement);
    assert_eq!(
        f.store.apply_legacy_policy_command(&denied).await,
        Err(IdentityStoreError::AccessDenied)
    );
    let replay = f.store.apply_legacy_policy_command(&cmd).await.unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.entry.after.policy.membership.principal_id, target);
    assert_eq!(
        f.store.legacy_access(scope(), replacement).await.unwrap(),
        None
    );
    let mut first_grant = command(manager, replacement);
    first_grant.expected_revision = None;
    let granted = f
        .store
        .apply_legacy_policy_command(&first_grant)
        .await
        .unwrap();
    assert!(granted.entry.before.is_none());
    assert_eq!(granted.entry.kind, PolicyAuditKind::Changed);
    assert_eq!(u64::from(granted.entry.after.revision), 1);
    assert_eq!(history(&f, manager, replacement).await.len(), 1);
    assert_eq!(history(&f, manager, target).await.len(), 2);
    f.cleanup().await;
}
