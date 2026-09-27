use cyanrex_engine::{
    models::collaboration::*,
    services::{
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            LegacyPolicyAuditEntry, LegacyPolicyCommand, LegacyPolicyResource, PolicyAuditKind,
            StoredLegacyAccessPolicy,
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
#[path = "collaboration_policy_audit/faults.rs"]
mod faults;
#[path = "collaboration_policy_audit/permissions.rs"]
mod permissions;

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

fn command(manager: PrincipalId, target: PrincipalId) -> LegacyPolicyCommand {
    LegacyPolicyCommand {
        command_id: id(),
        actor: actor(manager),
        expected_revision: Some(RevisionNumber::try_from(1).unwrap()),
        desired: policy(target, true),
    }
}

async fn seeded() -> (Fixture, PrincipalId, PrincipalId, PrincipalId) {
    let f = Fixture::ready().await;
    f.store.install_access_schema().await.unwrap();
    let mut principals = Vec::new();
    for name in ["manager-one", "manager-two", "synthetic-student"] {
        let identity = if name == "synthetic-student" {
            f.bind().await
        } else {
            f.store
                .bind_legacy_account(scope(), &name.parse().unwrap(), id())
                .await
                .unwrap()
        };
        let principal = identity.binding.principal_id;
        f.store
            .replace_legacy_access(None, &policy(principal, name != "synthetic-student"))
            .await
            .unwrap();
        principals.push(principal);
    }
    (f, principals[0], principals[1], principals[2])
}

async fn ready() -> (Fixture, PrincipalId, PrincipalId, PrincipalId) {
    let result = seeded().await;
    result.0.store.upgrade_policy_audit_schema().await.unwrap();
    result
}

async fn history(
    f: &Fixture,
    manager: PrincipalId,
    subject: PrincipalId,
) -> Vec<LegacyPolicyAuditEntry> {
    f.store
        .legacy_policy_audit(scope(), actor(manager), subject, None, 100)
        .await
        .unwrap()
}

async fn current(f: &Fixture, subject: PrincipalId) -> StoredLegacyAccessPolicy {
    f.store
        .legacy_access(scope(), subject)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn closed_policy_audit_storage_never_confirms_a_command_or_history() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let store = CollaborationIdentityStore::new(pool);
    assert_eq!(
        store.upgrade_policy_audit_schema().await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .apply_legacy_policy_command(&command(principal(), id()))
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .legacy_policy_audit(scope(), actor(principal()), id(), None, 10)
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_upgrade_is_explicit_repeatable_and_preserves_observed_baselines() {
    let (f, manager, _, target) = seeded().await;
    let cmd = command(manager, target);
    assert_eq!(
        f.store.apply_legacy_policy_command(&cmd).await,
        Err(IdentityStoreError::AuditNotEnabled)
    );
    let newer = f
        .store
        .replace_legacy_access(cmd.expected_revision, &cmd.desired)
        .await
        .unwrap();
    let peer = f.reopen().await;
    let (one, two) = tokio::join!(
        f.store.upgrade_policy_audit_schema(),
        peer.upgrade_policy_audit_schema()
    );
    one.unwrap();
    two.unwrap();
    f.store.install_access_schema().await.unwrap();
    assert_eq!(
        peer.legacy_access(scope(), target).await.unwrap(),
        Some(newer.clone())
    );
    assert_eq!(f.count("collaboration_policy_audit").await, 3);
    let entries = history(&f, manager, target).await;
    assert_eq!(entries.len(), 1);
    let baseline = &entries[0];
    assert_eq!(baseline.kind, PolicyAuditKind::Baseline);
    assert_eq!(baseline.after, newer);
    assert!(
        baseline.before.is_none()
            && baseline.actor.is_none()
            && baseline.command_id.is_none()
            && baseline.request_digest.is_none()
    );
    assert_eq!(
        f.store
            .replace_legacy_access(Some(newer.revision), &newer.policy)
            .await,
        Err(IdentityStoreError::AuditContextRequired)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_upgrade_rejects_unprepared_unknown_or_conflicting_storage() {
    let f = Fixture::ready().await;
    f.store.install_access_schema().await.unwrap();
    assert_eq!(
        f.store.upgrade_policy_audit_schema().await,
        Err(IdentityStoreError::AuditBootstrapRequired)
    );
    f.sql("UPDATE collaboration_access_schema SET version = 99")
        .await;
    assert_eq!(
        f.store.upgrade_policy_audit_schema().await,
        Err(IdentityStoreError::UnsupportedSchema)
    );
    f.cleanup().await;

    let (f, _, _, target) = seeded().await;
    let original = current(&f, target).await;
    f.sql("CREATE TABLE collaboration_policy_audit (sentinel TEXT)")
        .await;
    f.sql("INSERT INTO collaboration_policy_audit VALUES ('keep')")
        .await;
    assert!(f.store.upgrade_policy_audit_schema().await.is_err());
    assert_eq!(f.count("collaboration_policy_audit").await, 1);
    assert_eq!(current(&f, target).await, original);
    assert_eq!(
        query("SELECT version FROM collaboration_access_schema")
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
async fn postgres_audit_command_replay_returns_history_without_regranting() {
    let (f, manager, _, target) = ready().await;
    let original = current(&f, target).await;
    let cmd = command(manager, target);
    let applied = f.store.apply_legacy_policy_command(&cmd).await.unwrap();
    assert!(!applied.replayed);
    assert_eq!(applied.entry.kind, PolicyAuditKind::Changed);
    assert_eq!(applied.entry.actor, Some(actor(manager)));
    assert_eq!(applied.entry.command_id, Some(cmd.command_id));
    assert_eq!(applied.entry.before, Some(original));
    assert_eq!(applied.entry.after, current(&f, target).await);
    let mut revoke = command(manager, target);
    revoke.expected_revision = Some(applied.entry.after.revision);
    revoke.desired = policy(target, false);
    let revoked = f.store.apply_legacy_policy_command(&revoke).await.unwrap();
    let replay = f
        .reopen()
        .await
        .apply_legacy_policy_command(&cmd)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.entry, applied.entry);
    assert_eq!(current(&f, target).await, revoked.entry.after);
    assert_eq!(history(&f, manager, target).await.len(), 3);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_command_ids_bind_actor_revision_and_entire_desired_policy() {
    let (f, manager, backup, target) = ready().await;
    let cmd = command(manager, target);
    f.store.apply_legacy_policy_command(&cmd).await.unwrap();
    let mut variants = vec![cmd.clone(); 4];
    variants[0].actor = actor(backup);
    variants[1].expected_revision = None;
    variants[2].desired.deployment_granted = false;
    variants[3].desired.membership.principal_id = backup;
    for variant in variants {
        assert_eq!(
            f.store.apply_legacy_policy_command(&variant).await,
            Err(IdentityStoreError::CommandConflict)
        );
    }
    assert_eq!(f.count("collaboration_policy_audit").await, 4);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_concurrent_commands_deduplicate_and_reject_stale_revisions() {
    let (f, manager, _, target) = ready().await;
    let cmd = command(manager, target);
    let peer = f.reopen().await;
    let (one, two) = tokio::join!(
        f.store.apply_legacy_policy_command(&cmd),
        peer.apply_legacy_policy_command(&cmd)
    );
    let (one, two) = (one.unwrap(), two.unwrap());
    assert_ne!(one.replayed, two.replayed);
    assert_eq!(one.entry, two.entry);
    let mut first = command(manager, target);
    first.expected_revision = Some(one.entry.after.revision);
    first.desired = policy(target, false);
    let mut second = first.clone();
    second.command_id = id();
    let (one, two) = tokio::join!(
        f.store.apply_legacy_policy_command(&first),
        peer.apply_legacy_policy_command(&second)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert_eq!(
        one.err().or(two.err()),
        Some(IdentityStoreError::StaleAccess)
    );
    assert_eq!(f.count("collaboration_policy_audit").await, 5);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_audit_noop_receipts_are_durable_and_role_order_is_canonical() {
    let (f, manager, _, target) = ready().await;
    let mut cmd = command(manager, target);
    cmd.desired
        .membership
        .role_refs
        .push("cyanrex.workspace.owner".parse().unwrap());
    let applied = f.store.apply_legacy_policy_command(&cmd).await.unwrap();
    cmd.desired.membership.role_refs.reverse();
    assert_eq!(
        f.store
            .apply_legacy_policy_command(&cmd)
            .await
            .unwrap()
            .entry,
        applied.entry
    );
    cmd.command_id = id();
    cmd.expected_revision = Some(applied.entry.after.revision);
    let noop = f.store.apply_legacy_policy_command(&cmd).await.unwrap();
    assert_eq!(noop.entry.kind, PolicyAuditKind::Unchanged);
    assert_eq!(noop.entry.before, Some(applied.entry.after.clone()));
    assert_eq!(noop.entry.after, applied.entry.after);
    assert_eq!(history(&f, manager, target).await.len(), 3);
    f.cleanup().await;
}
