use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            AuthorityBootstrap, AuthorityReconciliation, DurableAuthSource, DurableRegistration,
            ReconciliationError, SessionBindCommand, SessionDeleteAccountCommand,
            SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, LegacyIdentityAction,
            LegacyIdentityCommand, StoredLegacyAccessPolicy, StoredLegacyIdentity,
        },
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;
#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::*;
#[path = "durable_reconciliation/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "durable_reconciliation/cli.rs"]
mod cli;
#[path = "durable_reconciliation/faults.rs"]
mod faults;

#[tokio::test]
async fn reconciliation_never_falls_back_or_accepts_a_foreign_authority() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1:1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool, authority());
    assert_eq!(
        source.reconcile_authority(scope()).await.unwrap_err(),
        ReconciliationError::StorageUnavailable
    );
    assert_eq!(
        source
            .reconcile_authority(WorkspaceRef {
                authority_id: id(),
                ..scope()
            })
            .await
            .unwrap_err(),
        ReconciliationError::ScopeMismatch
    );
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_bootstrap_is_consistent_secret_free_and_read_only() {
    let f = CheckFixture::new().await;
    let before = f.digest().await;
    let report = f.reconcile().await.unwrap();
    assert_eq!(report.workspace, scope());
    assert_eq!(report.accounts, 1);
    assert_eq!(report.bound_identities, 1);
    assert_eq!(report.current_managers, 1);
    assert_eq!(report.sessions, 0);
    assert_eq!(report.identity_audit_entries, 1);
    assert_eq!(report.policy_audit_entries, 1);
    assert_eq!(before, f.digest().await);
    let json = serde_json::to_string(&report).unwrap();
    for secret in [
        PASSWORD,
        f.owner.registration.bootstrap.secret.as_str(),
        "password_hash",
        "totp_secret",
        "token",
    ] {
        assert!(!json.contains(secret));
    }
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_unbound_disabled_retired_and_expired_states_are_distinct() {
    let f = CheckFixture::new().await;
    let extra =
        f.db.source
            .register(&"unbound-source".parse().unwrap(), PASSWORD)
            .await
            .unwrap();
    f.db.source
        .login(
            &extra.account.username,
            PASSWORD,
            &otp(&extra.bootstrap.secret),
        )
        .await
        .unwrap();
    f.db.sql("UPDATE sessions SET expires_at=NOW()-interval '1 second'")
        .await;
    let (_, bound) = f.bound("retired-source").await;
    f.policy(bound.binding.principal_id, true).await;
    let registry = CollaborationIdentityStore::new(f.db.pool.clone());
    registry
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.owner.identity.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: bound.binding.username.clone(),
                account_id: bound.account_id,
                expected_principal: bound.binding.principal_id,
            },
        })
        .await
        .unwrap();
    let before = f.digest().await;
    let report = f.reconcile().await.unwrap();
    assert_eq!(report.accounts, 3);
    assert_eq!(report.unbound_accounts, 1);
    assert_eq!(report.retired_accounts_present, 1);
    assert_eq!(report.retired_identities, 1);
    assert_eq!(report.expired_sessions, 1);
    assert_eq!(
        report.current_managers, 1,
        "retained grant on retired identity is not authority"
    );
    assert_eq!(before, f.digest().await);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_deletion_recreation_and_password_rotation_preserve_history() {
    let f = CheckFixture::new().await;
    let (created, binding) = f.bound("replaceable-user").await;
    f.policy(binding.binding.principal_id, false).await;
    let token = f.owner_login().await;
    f.db.source
        .delete_session_account(
            &token,
            &SessionDeleteAccountCommand {
                command_id: id(),
                workspace: scope(),
                target: created.account.clone(),
                expected_principal: binding.binding.principal_id,
            },
        )
        .await
        .unwrap();
    let replacement =
        f.db.source
            .register(&created.account.username, PASSWORD)
            .await
            .unwrap();
    assert_ne!(replacement.account.account_id, created.account.account_id);
    let report = f.reconcile().await.unwrap();
    assert_eq!(report.retired_identities, 1);
    assert_eq!(report.retired_accounts_present, 0);
    assert_eq!(report.unbound_accounts, 1);
    assert_eq!(report.current_managers, 1);
    f.db.source
        .change_session_password(
            &token,
            PASSWORD,
            "synthetic-new-password",
            &next_otp(&f.owner.registration.bootstrap.secret),
        )
        .await
        .unwrap();
    let report = f.reconcile().await.unwrap();
    assert_eq!(report.sessions, 0);
    assert_eq!(report.identity_audit_entries, 3);
    assert_eq!(report.policy_audit_entries, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires explicit disposable PostgreSQL"]
async fn postgres_reconcile_management_requires_source_binding_and_explicit_grant_not_roles() {
    let f = CheckFixture::new().await;
    let (_, learner) = f.bound("new-manager").await;
    f.policy(learner.binding.principal_id, true).await;
    f.policy(f.owner.identity.binding.principal_id, false).await;
    f.db.sql("UPDATE collaboration_workspaces SET status='archived'")
        .await;
    let report = f.reconcile().await.unwrap();
    assert_eq!(
        report.current_managers, 1,
        "suspended learner still has explicit instance management"
    );
    assert_eq!(report.workspace_status, WorkspaceStatus::Archived);
    // An unaudited direct change must not become a silently accepted manager observation.
    f.db.sql("UPDATE collaboration_deployment_grants SET status='revoked'")
        .await;
    assert_eq!(
        f.reconcile().await.unwrap_err(),
        ReconciliationError::PolicyHistoryMismatch
    );
    f.cleanup().await;
}
