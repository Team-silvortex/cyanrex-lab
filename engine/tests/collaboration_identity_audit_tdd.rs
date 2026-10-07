use cyanrex_engine::{
    models::collaboration::*,
    services::{
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            LegacyIdentityAction, LegacyIdentityAuditEntry, LegacyIdentityAuditKind,
            LegacyIdentityCommand, LegacyPolicyCommand, LegacyPolicyResource, StoredLegacyIdentity,
        },
        legacy_workspace::LegacyAction,
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::PgPoolOptions;

#[allow(dead_code)]
#[path = "collaboration_identity_store/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "collaboration_identity_audit/faults.rs"]
mod faults;
#[path = "collaboration_identity_audit/permissions.rs"]
mod permissions;
#[path = "collaboration_identity_audit/timestamps.rs"]
mod timestamps;

fn actor(principal_id: PrincipalId) -> PrincipalRef {
    PrincipalRef {
        authority_id: scope().authority_id,
        principal_id,
    }
}

fn policy(principal_id: PrincipalId, grant: bool) -> LegacyAccessPolicy {
    LegacyAccessPolicy {
        membership: Membership {
            workspace: scope(),
            principal_id,
            role_refs: vec![if grant {
                "cyanrex.teaching.teacher"
            } else {
                "cyanrex.teaching.learner"
            }
            .parse()
            .unwrap()],
            status: MembershipStatus::Active,
        },
        deployment_granted: grant,
    }
}

fn bind(manager: PrincipalId) -> LegacyIdentityCommand {
    LegacyIdentityCommand {
        command_id: id(),
        actor: actor(manager),
        workspace: scope(),
        action: LegacyIdentityAction::Bind {
            username: "new-student".parse().unwrap(),
            account_id: id(),
        },
    }
}

fn retire(manager: PrincipalId, identity: &StoredLegacyIdentity) -> LegacyIdentityCommand {
    LegacyIdentityCommand {
        command_id: id(),
        actor: actor(manager),
        workspace: scope(),
        action: LegacyIdentityAction::Retire {
            username: identity.binding.username.clone(),
            account_id: identity.account_id,
            expected_principal: identity.binding.principal_id,
        },
    }
}

async fn seeded() -> (
    Fixture,
    StoredLegacyIdentity,
    StoredLegacyIdentity,
    StoredLegacyIdentity,
) {
    let f = Fixture::ready().await;
    f.store.install_access_schema().await.unwrap();
    let mut identities = Vec::new();
    for name in ["manager-one", "manager-two", "synthetic-student"] {
        let identity = if name == "synthetic-student" {
            f.bind().await
        } else {
            f.store
                .bind_legacy_account(scope(), &name.parse().unwrap(), id())
                .await
                .unwrap()
        };
        f.store
            .replace_legacy_access(
                None,
                &policy(identity.binding.principal_id, name != "synthetic-student"),
            )
            .await
            .unwrap();
        identities.push(identity);
    }
    f.store.upgrade_policy_audit_schema().await.unwrap();
    (
        f,
        identities.remove(0),
        identities.remove(0),
        identities.remove(0),
    )
}

async fn ready() -> (
    Fixture,
    StoredLegacyIdentity,
    StoredLegacyIdentity,
    StoredLegacyIdentity,
) {
    let result = seeded().await;
    result
        .0
        .store
        .upgrade_identity_audit_schema()
        .await
        .unwrap();
    result
}

async fn current(f: &Fixture, expected: &StoredLegacyIdentity) -> StoredLegacyIdentity {
    f.store
        .lookup_legacy_account(scope(), &expected.binding.username, expected.account_id)
        .await
        .unwrap()
        .unwrap()
}

async fn history(
    f: &Fixture,
    manager: PrincipalId,
    subject: PrincipalId,
) -> Vec<LegacyIdentityAuditEntry> {
    f.store
        .legacy_identity_audit(scope(), actor(manager), subject, None, 100)
        .await
        .unwrap()
}

#[tokio::test]
async fn closed_identity_audit_storage_never_confirms_a_command_or_history() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let store = CollaborationIdentityStore::new(pool);
    assert_eq!(
        store.upgrade_identity_audit_schema().await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .apply_legacy_identity_command(&bind(principal()))
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .legacy_identity_audit(scope(), actor(principal()), id(), None, 10)
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_upgrade_is_explicit_repeatable_and_fences_unattributed_writers() {
    let (f, manager, _, target) = seeded().await;
    assert_eq!(
        f.store
            .apply_legacy_identity_command(&bind(manager.binding.principal_id))
            .await,
        Err(IdentityStoreError::IdentityAuditNotEnabled)
    );
    let peer = f.reopen().await;
    let (one, two) = tokio::join!(
        f.store.upgrade_identity_audit_schema(),
        peer.upgrade_identity_audit_schema()
    );
    one.unwrap();
    two.unwrap();
    f.store.install_schema().await.unwrap();
    f.store.install_access_schema().await.unwrap();
    f.store.upgrade_policy_audit_schema().await.unwrap();
    assert_eq!(f.count("collaboration_identity_audit").await, 3);
    let entries = history(
        &f,
        manager.binding.principal_id,
        target.binding.principal_id,
    )
    .await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, LegacyIdentityAuditKind::Baseline);
    assert_eq!(entries[0].after, target);
    assert!(
        entries[0].before.is_none()
            && entries[0].command_id.is_none()
            && entries[0].actor.is_none()
            && entries[0].action.is_none()
            && entries[0].request_digest.is_none()
    );
    assert_eq!(
        peer.lookup_legacy_account(scope(), &username(), account())
            .await
            .unwrap(),
        Some(target.clone())
    );
    assert_eq!(
        f.store
            .bind_legacy_account(scope(), &username(), id())
            .await,
        Err(IdentityStoreError::IdentityAuditContextRequired)
    );
    assert_eq!(
        f.store
            .retire_legacy_account(scope(), &username(), account(), target.binding.principal_id)
            .await,
        Err(IdentityStoreError::IdentityAuditContextRequired)
    );
    assert_eq!(
        f.store
            .provision_legacy_workspace(WorkspaceRef {
                authority_id: id(),
                workspace_id: id()
            })
            .await,
        Err(IdentityStoreError::IdentityAuditContextRequired)
    );
    assert_eq!(current(&f, &target).await, target);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_upgrade_requires_policy_audit_and_effective_managers() {
    let f = Fixture::ready().await;
    f.store.install_access_schema().await.unwrap();
    assert_eq!(
        f.store.upgrade_identity_audit_schema().await,
        Err(IdentityStoreError::AuditNotEnabled)
    );
    f.sql("UPDATE collaboration_identity_schema SET version = 99")
        .await;
    assert_eq!(
        f.store.upgrade_identity_audit_schema().await,
        Err(IdentityStoreError::UnsupportedSchema)
    );
    f.cleanup().await;
    let (f, _, _, _) = seeded().await;
    f.sql("UPDATE collaboration_principals SET status = 'disabled'")
        .await;
    assert_eq!(
        f.store.upgrade_identity_audit_schema().await,
        Err(IdentityStoreError::AuditBootstrapRequired)
    );
    assert_eq!(
        query("SELECT version FROM collaboration_identity_schema")
            .fetch_one(&f.pool)
            .await
            .unwrap()
            .get::<i32, _>("version"),
        1
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_bind_retire_recreate_and_replay_never_reuse_an_incarnation() {
    let (f, manager, _, _) = ready().await;
    let manager = manager.binding.principal_id;
    let cmd = bind(manager);
    let bound = f.store.apply_legacy_identity_command(&cmd).await.unwrap();
    assert_eq!(bound.entry.kind, LegacyIdentityAuditKind::Bound);
    assert!(bound.entry.before.is_none());
    let old = &bound.entry.after;
    assert_eq!(bound.entry.actor, Some(actor(manager)));
    assert_eq!(
        f.store
            .legacy_access(scope(), old.binding.principal_id)
            .await
            .unwrap(),
        None
    );
    let retirement = retire(manager, old);
    let retired = f
        .store
        .apply_legacy_identity_command(&retirement)
        .await
        .unwrap();
    assert_eq!(retired.entry.kind, LegacyIdentityAuditKind::Retired);
    assert_eq!(retired.entry.before.as_ref(), Some(old));
    assert!(retired.entry.after.retired_at.is_some());
    assert_eq!(
        retired.entry.after.principal.status,
        PrincipalStatus::Disabled
    );
    let replacement = f
        .store
        .apply_legacy_identity_command(&bind(manager))
        .await
        .unwrap()
        .entry
        .after;
    assert_ne!(replacement.binding.principal_id, old.binding.principal_id);
    assert_eq!(
        f.store
            .legacy_access(scope(), replacement.binding.principal_id)
            .await
            .unwrap(),
        None
    );
    let replay = f
        .reopen()
        .await
        .apply_legacy_identity_command(&cmd)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.entry, bound.entry);
    let repeated = f
        .store
        .apply_legacy_identity_command(&retire(manager, old))
        .await
        .unwrap();
    assert_eq!(repeated.entry.kind, LegacyIdentityAuditKind::Unchanged);
    assert_eq!(repeated.entry.after, retired.entry.after);
    assert_eq!(current(&f, &replacement).await, replacement);
    let mut stale = retire(manager, &replacement);
    if let LegacyIdentityAction::Retire {
        expected_principal, ..
    } = &mut stale.action
    {
        *expected_principal = old.binding.principal_id;
    }
    assert_eq!(
        f.store.apply_legacy_identity_command(&stale).await,
        Err(IdentityStoreError::StaleIdentity)
    );
    assert_eq!(
        history(&f, manager, old.binding.principal_id).await.len(),
        3
    );
    assert_eq!(
        history(&f, manager, replacement.binding.principal_id)
            .await
            .len(),
        1
    );
    let grant = LegacyPolicyCommand {
        command_id: id(),
        actor: actor(manager),
        expected_revision: None,
        desired: policy(replacement.binding.principal_id, true),
    };
    f.store.apply_legacy_policy_command(&grant).await.unwrap();
    assert!(f
        .store
        .preview_legacy_access(
            scope(),
            actor(replacement.binding.principal_id),
            LegacyAction::ManageDeployment,
            &LegacyPolicyResource::Deployment {
                authority_id: scope().authority_id
            }
        )
        .await
        .unwrap());
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_command_ids_bind_actor_scope_and_action() {
    let (f, manager, backup, _) = ready().await;
    let cmd = bind(manager.binding.principal_id);
    let original = f.store.apply_legacy_identity_command(&cmd).await.unwrap();
    let mut variants = vec![cmd.clone(); 4];
    variants[0].actor = actor(backup.binding.principal_id);
    variants[1].action = retire(manager.binding.principal_id, &original.entry.after).action;
    variants[2].action = bind(manager.binding.principal_id).action;
    if let LegacyIdentityAction::Bind { username, .. } = &mut variants[3].action {
        *username = "other-student".parse().unwrap();
    }
    for variant in variants {
        assert_eq!(
            f.store.apply_legacy_identity_command(&variant).await,
            Err(IdentityStoreError::CommandConflict)
        );
    }
    let mut wrong_scope = cmd.clone();
    wrong_scope.workspace.workspace_id = id();
    assert_eq!(
        f.store.apply_legacy_identity_command(&wrong_scope).await,
        Err(IdentityStoreError::ScopeConflict)
    );
    assert_eq!(f.count("collaboration_identity_audit").await, 4);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_lifecycle_concurrent_commands_converge_and_record_noop_receipts() {
    let (f, manager, _, _) = ready().await;
    let cmd = bind(manager.binding.principal_id);
    let peer = f.reopen().await;
    let (one, two) = tokio::join!(
        f.store.apply_legacy_identity_command(&cmd),
        peer.apply_legacy_identity_command(&cmd)
    );
    let (one, two) = (one.unwrap(), two.unwrap());
    assert_ne!(one.replayed, two.replayed);
    assert_eq!(one.entry, two.entry);
    let mut repeated = cmd.clone();
    repeated.command_id = id();
    let unchanged = f
        .store
        .apply_legacy_identity_command(&repeated)
        .await
        .unwrap();
    assert_eq!(unchanged.entry.kind, LegacyIdentityAuditKind::Unchanged);
    assert_eq!(unchanged.entry.before.as_ref(), Some(&one.entry.after));
    assert_eq!(unchanged.entry.after, one.entry.after);
    assert_eq!(f.count("collaboration_principals").await, 4);
    assert_eq!(f.count("collaboration_identity_audit").await, 5);
    f.cleanup().await;
}
