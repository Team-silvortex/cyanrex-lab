use cyanrex_engine::{
    models::collaboration::*,
    services::{
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            LegacyPolicyResource, StoredLegacyAccessPolicy,
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

fn policy(principal_id: PrincipalId, role: &str, deployment_granted: bool) -> LegacyAccessPolicy {
    LegacyAccessPolicy {
        membership: Membership {
            workspace: scope(),
            principal_id,
            role_refs: vec![role.parse().unwrap()],
            status: MembershipStatus::Active,
        },
        deployment_granted,
    }
}

fn actor(principal_id: PrincipalId) -> PrincipalRef {
    PrincipalRef {
        authority_id: scope().authority_id,
        principal_id,
    }
}

fn deployment() -> LegacyPolicyResource {
    LegacyPolicyResource::Deployment {
        authority_id: scope().authority_id,
    }
}

async fn ready() -> Fixture {
    let f = Fixture::ready().await;
    f.store.install_access_schema().await.unwrap();
    f
}

async fn member(f: &Fixture, name: &str, role: &str, grant: bool) -> StoredLegacyAccessPolicy {
    let identity = f
        .store
        .bind_legacy_account(scope(), &name.parse().unwrap(), id())
        .await
        .unwrap();
    f.store
        .replace_legacy_access(None, &policy(identity.binding.principal_id, role, grant))
        .await
        .unwrap()
}

#[tokio::test]
async fn closed_access_storage_never_returns_permissions_or_memory_success() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let store = CollaborationIdentityStore::new(pool);
    assert_eq!(
        store.install_access_schema().await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.legacy_access(scope(), principal()).await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .replace_legacy_access(None, &policy(principal(), "cyanrex.teaching.teacher", true))
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .preview_legacy_access(
                scope(),
                actor(principal()),
                LegacyAction::ManageDeployment,
                &deployment()
            )
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_schema_is_explicit_repeatable_and_fail_closed() {
    let f = Fixture::new().await;
    assert!(f.store.install_access_schema().await.is_err());
    assert_eq!(
        f.store.legacy_access(scope(), principal()).await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    f.store.install_schema().await.unwrap();
    f.store.provision_legacy_workspace(scope()).await.unwrap();
    let (one, two) = tokio::join!(
        f.store.install_access_schema(),
        f.store.install_access_schema()
    );
    one.unwrap();
    two.unwrap();
    assert_eq!(
        f.store.legacy_access(scope(), principal()).await.unwrap(),
        None
    );
    assert_eq!(f.count("collaboration_memberships").await, 0);
    f.sql("UPDATE collaboration_access_schema SET version = 99")
        .await;
    assert_eq!(
        f.store.install_access_schema().await,
        Err(IdentityStoreError::UnsupportedSchema)
    );
    assert_eq!(
        f.store.legacy_access(scope(), principal()).await,
        Err(IdentityStoreError::UnsupportedSchema)
    );
    f.sql("UPDATE collaboration_access_schema SET version = 1")
        .await;
    f.sql("ALTER TABLE collaboration_memberships RENAME COLUMN role_refs TO broken_roles")
        .await;
    assert!(f.store.install_access_schema().await.is_err());
    assert!(f.store.legacy_access(scope(), principal()).await.is_err());
    f.sql("ALTER TABLE collaboration_memberships RENAME COLUMN broken_roles TO role_refs")
        .await;
    f.store.install_access_schema().await.unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_policy_reload_and_compare_swap_prevent_stale_regrant() {
    let f = ready().await;
    let original = member(&f, "teacher-one", "cyanrex.teaching.teacher", true).await;
    assert_eq!(u64::from(original.revision), 1);
    let principal = original.policy.membership.principal_id;
    let peer = f.reopen().await;
    assert_eq!(
        peer.legacy_access(scope(), principal).await.unwrap(),
        Some(original.clone())
    );
    assert_eq!(
        peer.replace_legacy_access(Some(original.revision), &original.policy)
            .await
            .unwrap(),
        original
    );
    let mut revoked = original.policy.clone();
    revoked.membership.status = MembershipStatus::Suspended;
    revoked.deployment_granted = false;
    let newer = peer
        .replace_legacy_access(Some(original.revision), &revoked)
        .await
        .unwrap();
    assert_eq!(u64::from(newer.revision), 2);
    for revision in [None, Some(original.revision)] {
        assert_eq!(
            f.store
                .replace_legacy_access(revision, &original.policy)
                .await,
            Err(IdentityStoreError::StaleAccess)
        );
    }
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &deployment()
        )
        .await
        .unwrap());
    assert_eq!(
        f.store.legacy_access(scope(), principal).await.unwrap(),
        Some(newer)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_concurrent_writers_do_not_overwrite_reviewed_revisions() {
    let f = ready().await;
    let identity = f.bind().await;
    let teacher = policy(
        identity.binding.principal_id,
        "cyanrex.teaching.teacher",
        true,
    );
    let learner = policy(
        identity.binding.principal_id,
        "cyanrex.teaching.learner",
        false,
    );
    let peer = f.reopen().await;
    let (one, two) = tokio::join!(
        f.store.replace_legacy_access(None, &teacher),
        peer.replace_legacy_access(None, &learner)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert_eq!(
        one.err().or(two.err()),
        Some(IdentityStoreError::StaleAccess)
    );
    let current = f
        .store
        .legacy_access(scope(), identity.binding.principal_id)
        .await
        .unwrap()
        .unwrap();
    let mut suspended = current.policy.clone();
    suspended.membership.status = MembershipStatus::Suspended;
    suspended.deployment_granted = false;
    let mut owner = policy(
        identity.binding.principal_id,
        "cyanrex.workspace.owner",
        false,
    );
    owner.membership.status = MembershipStatus::Active;
    let (one, two) = tokio::join!(
        f.store
            .replace_legacy_access(Some(current.revision), &suspended),
        peer.replace_legacy_access(Some(current.revision), &owner)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert_eq!(
        one.err().or(two.err()),
        Some(IdentityStoreError::StaleAccess)
    );
    assert_eq!(f.count("collaboration_memberships").await, 1);
    assert_eq!(f.count("collaboration_deployment_grants").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_access_scope_and_unknown_identities_never_gain_access() {
    let f = ready().await;
    let original = member(&f, "teacher-one", "cyanrex.teaching.teacher", true).await;
    let principal = original.policy.membership.principal_id;
    let wrong_scope = WorkspaceRef {
        workspace_id: id(),
        ..scope()
    };
    let foreign = WorkspaceRef {
        authority_id: id(),
        ..scope()
    };
    f.store.provision_legacy_workspace(foreign).await.unwrap();
    let unassigned = f.bind().await.binding.principal_id;
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(unassigned),
            LegacyAction::ReadPrivateArtifact,
            &LegacyPolicyResource::PrivateArtifact {
                workspace: scope(),
                owner_id: unassigned
            }
        )
        .await
        .unwrap());
    assert_eq!(
        f.store.legacy_access(wrong_scope, principal).await,
        Err(IdentityStoreError::ScopeConflict)
    );
    let mut misplaced = original.policy.clone();
    misplaced.membership.workspace = foreign;
    assert_eq!(
        f.store.replace_legacy_access(None, &misplaced).await,
        Err(IdentityStoreError::StaleIdentity)
    );
    for who in [
        actor(id()),
        PrincipalRef {
            authority_id: foreign.authority_id,
            principal_id: principal,
        },
    ] {
        assert!(!f
            .store
            .preview_legacy_access(scope(), who, LegacyAction::ManageDeployment, &deployment())
            .await
            .unwrap());
    }
    for other_scope in [wrong_scope, foreign] {
        assert!(!f
            .store
            .preview_legacy_access(
                scope(),
                actor(principal),
                LegacyAction::ReadPrivateArtifact,
                &LegacyPolicyResource::PrivateArtifact {
                    workspace: other_scope,
                    owner_id: principal
                }
            )
            .await
            .unwrap());
    }
    assert!(!f
        .store
        .preview_legacy_access(
            scope(),
            actor(principal),
            LegacyAction::ManageDeployment,
            &LegacyPolicyResource::Deployment {
                authority_id: foreign.authority_id
            }
        )
        .await
        .unwrap());
    assert_eq!(f.count("collaboration_memberships").await, 1);
    f.cleanup().await;
}

#[path = "collaboration_access_store/faults.rs"]
mod faults;

#[path = "collaboration_access_store/policy.rs"]
mod policy;
