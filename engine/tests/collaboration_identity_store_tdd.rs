use cyanrex_engine::{
    models::collaboration::*,
    services::collaboration_identity_store::{CollaborationIdentityStore, IdentityStoreError},
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::PgPoolOptions;

#[path = "collaboration_identity_store/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "collaboration_identity_store/timestamps.rs"]
mod timestamps;

#[tokio::test]
async fn closed_identity_storage_never_falls_back_or_allocates_on_read() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let store = CollaborationIdentityStore::new(pool);
    let scope = scope();
    assert_eq!(
        store.install_schema().await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.legacy_workspace(scope.authority_id).await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store.provision_legacy_workspace(scope).await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .bind_legacy_account(scope, &username(), account())
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .lookup_legacy_account(scope, &username(), account())
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        store
            .retire_legacy_account(scope, &username(), account(), principal())
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_schema_and_bootstrap_are_repeatable_and_concurrent() {
    let f = Fixture::new().await;
    let peer = CollaborationIdentityStore::new(f.pool.clone());
    let (one, two) = tokio::join!(f.store.install_schema(), peer.install_schema());
    one.unwrap();
    two.unwrap();
    assert_eq!(
        f.store
            .legacy_workspace(scope().authority_id)
            .await
            .unwrap(),
        None
    );
    let (one, two) = tokio::join!(
        f.store.provision_legacy_workspace(scope()),
        peer.provision_legacy_workspace(scope())
    );
    let expected = one.unwrap();
    assert_eq!(expected, two.unwrap());
    assert_eq!(expected.reference, scope());
    assert_eq!(expected.status, WorkspaceStatus::Active);
    let reopened = f.reopen().await;
    assert_eq!(
        reopened
            .legacy_workspace(scope().authority_id)
            .await
            .unwrap(),
        Some(expected)
    );
    assert_eq!(f.count("collaboration_authorities").await, 1);
    assert_eq!(f.count("collaboration_workspaces").await, 1);
    assert_eq!(f.count("collaboration_legacy_workspaces").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_bootstrap_rejects_rebinding_and_preserves_archived_workspace() {
    let f = Fixture::ready().await;
    let other = WorkspaceRef {
        workspace_id: id(),
        ..scope()
    };
    assert_eq!(
        f.store.provision_legacy_workspace(other).await,
        Err(IdentityStoreError::ScopeConflict)
    );
    assert_eq!(f.count("collaboration_workspaces").await, 1);
    f.sql("UPDATE collaboration_workspaces SET status = 'archived'")
        .await;
    assert_eq!(
        f.store.provision_legacy_workspace(scope()).await,
        Err(IdentityStoreError::ScopeInactive)
    );
    assert_eq!(
        f.store
            .bind_legacy_account(scope(), &username(), account())
            .await,
        Err(IdentityStoreError::ScopeInactive)
    );
    assert_eq!(
        f.store
            .legacy_workspace(scope().authority_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        WorkspaceStatus::Archived
    );
    assert_eq!(f.count("collaboration_principals").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_binding_survives_reload_and_converges_without_duplicate_principals() {
    let f = Fixture::ready().await;
    let peer = f.reopen().await;
    let username = username();
    let generation = account();
    assert_eq!(
        peer.lookup_legacy_account(scope(), &username, generation)
            .await
            .unwrap(),
        None
    );
    assert_eq!(f.count("collaboration_principals").await, 0);
    let (one, two) = tokio::join!(
        f.store.bind_legacy_account(scope(), &username, generation),
        peer.bind_legacy_account(scope(), &username, generation)
    );
    let original = one.unwrap();
    assert_eq!(original, two.unwrap());
    assert_eq!(original.account_id, generation);
    assert!(original.retired_at.is_none());
    assert_eq!(original.principal.kind, PrincipalKind::Human);
    assert_eq!(original.principal.status, PrincipalStatus::Active);
    assert_eq!(
        original.binding.principal_id,
        original.principal.reference.principal_id
    );
    assert_eq!(
        peer.lookup_legacy_account(scope(), &username, generation)
            .await
            .unwrap(),
        Some(original)
    );
    assert_eq!(f.count("collaboration_principals").await, 1);
    assert_eq!(f.count("collaboration_legacy_identities").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_conflicting_generations_and_reused_account_ids_fail_closed() {
    let f = Fixture::ready().await;
    let username = username();
    let first = account();
    let second = id();
    let (one, two) = tokio::join!(
        f.store.bind_legacy_account(scope(), &username, first),
        f.store.bind_legacy_account(scope(), &username, second)
    );
    let winner = match (one, two) {
        (Ok(value), Err(IdentityStoreError::IdentityConflict))
        | (Err(IdentityStoreError::IdentityConflict), Ok(value)) => value,
        other => panic!("only one generation may become active: {other:?}"),
    };
    assert_eq!(
        f.store
            .bind_legacy_account(scope(), &"another-user".parse().unwrap(), winner.account_id)
            .await,
        Err(IdentityStoreError::IdentityConflict)
    );
    assert_eq!(f.count("collaboration_principals").await, 1);
    assert_eq!(f.count("collaboration_legacy_identities").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_retirement_keeps_history_and_never_revives_a_recreated_name() {
    let f = Fixture::ready().await;
    let original = f.bind().await;
    let retired = f
        .store
        .retire_legacy_account(
            scope(),
            &username(),
            account(),
            original.binding.principal_id,
        )
        .await
        .unwrap();
    assert!(retired.retired_at.is_some());
    assert_eq!(retired.principal.status, PrincipalStatus::Disabled);
    assert_eq!(
        f.store
            .bind_legacy_account(scope(), &username(), account())
            .await,
        Err(IdentityStoreError::IdentityRetired)
    );
    let next_generation = id();
    let recreated = f
        .store
        .bind_legacy_account(scope(), &username(), next_generation)
        .await
        .unwrap();
    assert_ne!(
        original.binding.principal_id,
        recreated.binding.principal_id
    );
    assert_eq!(
        f.store
            .retire_legacy_account(
                scope(),
                &username(),
                account(),
                original.binding.principal_id
            )
            .await
            .unwrap(),
        retired
    );
    assert_eq!(
        f.store
            .lookup_legacy_account(scope(), &username(), next_generation)
            .await
            .unwrap(),
        Some(recreated)
    );
    assert_eq!(f.count("collaboration_principals").await, 2);
    assert_eq!(f.count("collaboration_legacy_identities").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_scope_and_expected_principal_cannot_target_another_account() {
    let f = Fixture::ready().await;
    let original = f.bind().await;
    let other = WorkspaceRef {
        authority_id: id(),
        ..scope()
    };
    f.store.provision_legacy_workspace(other).await.unwrap();
    let foreign = f
        .store
        .bind_legacy_account(other, &username(), account())
        .await
        .unwrap();
    assert_ne!(original.principal.reference, foreign.principal.reference);
    assert_eq!(
        f.store
            .retire_legacy_account(
                scope(),
                &username(),
                account(),
                foreign.binding.principal_id
            )
            .await,
        Err(IdentityStoreError::StaleIdentity)
    );
    let unbound = WorkspaceRef {
        workspace_id: id(),
        ..scope()
    };
    assert_eq!(
        f.store
            .lookup_legacy_account(unbound, &username(), account())
            .await,
        Err(IdentityStoreError::ScopeConflict)
    );
    assert_eq!(
        f.store
            .bind_legacy_account(unbound, &username(), account())
            .await,
        Err(IdentityStoreError::ScopeConflict)
    );
    assert_eq!(
        f.store
            .lookup_legacy_account(scope(), &username(), account())
            .await
            .unwrap(),
        Some(original)
    );
    assert_eq!(
        f.store
            .lookup_legacy_account(other, &username(), account())
            .await
            .unwrap(),
        Some(foreign)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_missing_or_unknown_schema_is_not_empty_or_repaired_silently() {
    let f = Fixture::new().await;
    assert_eq!(
        f.store.legacy_workspace(scope().authority_id).await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    f.store.install_schema().await.unwrap();
    f.sql("UPDATE collaboration_identity_schema SET version = 99")
        .await;
    assert_eq!(
        f.store.install_schema().await,
        Err(IdentityStoreError::UnsupportedSchema)
    );
    assert_eq!(
        f.store.provision_legacy_workspace(scope()).await,
        Err(IdentityStoreError::UnsupportedSchema)
    );
    assert_eq!(
        f.store.legacy_workspace(scope().authority_id).await,
        Err(IdentityStoreError::UnsupportedSchema)
    );
    assert_eq!(f.count("collaboration_authorities").await, 0);
    f.sql("UPDATE collaboration_identity_schema SET version = 1")
        .await;
    f.store.provision_legacy_workspace(scope()).await.unwrap();
    let original = f.bind().await;
    f.sql("ALTER TABLE collaboration_principals RENAME COLUMN status TO broken_status")
        .await;
    assert_eq!(
        f.store.install_schema().await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    assert_eq!(
        f.store
            .lookup_legacy_account(scope(), &username(), account())
            .await,
        Err(IdentityStoreError::StorageUnavailable)
    );
    f.sql("ALTER TABLE collaboration_principals RENAME COLUMN broken_status TO status")
        .await;
    assert_eq!(
        f.store
            .lookup_legacy_account(scope(), &username(), account())
            .await
            .unwrap(),
        Some(original)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_identity_disabled_or_inconsistent_records_cannot_be_rebound() {
    let f = Fixture::ready().await;
    let original = f.bind().await;
    f.sql("UPDATE collaboration_principals SET status = 'disabled'")
        .await;
    assert_eq!(
        f.store
            .bind_legacy_account(scope(), &username(), account())
            .await,
        Err(IdentityStoreError::IdentityInactive)
    );
    f.store
        .retire_legacy_account(
            scope(),
            &username(),
            account(),
            original.binding.principal_id,
        )
        .await
        .unwrap();
    f.sql("UPDATE collaboration_principals SET status = 'active'")
        .await;
    assert_eq!(
        f.store
            .lookup_legacy_account(scope(), &username(), account())
            .await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store
            .bind_legacy_account(scope(), &username(), account())
            .await,
        Err(IdentityStoreError::InvalidRecord)
    );
    assert_eq!(f.count("collaboration_principals").await, 1);
    f.cleanup().await;
}

#[path = "collaboration_identity_store/faults.rs"]
mod faults;
