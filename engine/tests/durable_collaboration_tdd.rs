use chrono::{Duration, Utc};
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            LegacyIdentityAction, LegacyIdentityCommand, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::{authority, id, otp, PASSWORD};
#[path = "durable_collaboration/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "durable_collaboration/faults.rs"]
mod faults;

#[tokio::test]
async fn closed_session_commands_never_use_a_detached_identity_or_memory() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool, authority());
    let cmd = SessionBindCommand {
        command_id: id(),
        workspace: scope(),
        target: DurableAccountRef {
            authority_id: authority(),
            username: "synthetic-user".parse().unwrap(),
            account_id: id(),
        },
    };
    let token = Uuid::new_v4().to_string();
    assert_eq!(
        source.bind_session_account(&token, &cmd).await,
        Err(SessionCommandError::Authentication(
            DurableAuthError::StorageUnavailable
        ))
    );
    assert_eq!(
        source
            .apply_session_policy_command(&token, &policy_command(id(), false))
            .await,
        Err(SessionCommandError::Authentication(
            DurableAuthError::StorageUnavailable
        ))
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_require_explicit_source_and_both_audit_schemas() {
    let f = Fixture::seeded().await;
    let cmd = f.bind_command();
    assert_eq!(
        f.source.bind_session_account(&f.manager_token, &cmd).await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::IdentityAuditNotEnabled
        ))
    );
    f.store.upgrade_policy_audit_schema().await.unwrap();
    assert_eq!(
        f.source.bind_session_account(&f.manager_token, &cmd).await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::IdentityAuditNotEnabled
        ))
    );
    f.store.upgrade_identity_audit_schema().await.unwrap();
    f.source
        .bind_session_account(&f.manager_token, &cmd)
        .await
        .unwrap();
    f.sql("UPDATE collaboration_access_schema SET version = 999")
        .await;
    assert_eq!(
        f.source.bind_session_account(&f.manager_token, &cmd).await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::UnsupportedSchema
        ))
    );
    f.cleanup().await;

    let f = source_fixture::Fixture::new().await;
    assert!(f
        .source
        .apply_session_policy_command(&Uuid::new_v4().to_string(), &policy_command(id(), false))
        .await
        .is_err());
    assert_eq!(
        f.table_count().await,
        0,
        "command must not install or seed anything"
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_derive_actor_and_verify_exact_target_incarnation() {
    let f = Fixture::ready().await;
    let cmd = f.bind_command();
    for target in [
        DurableAccountRef {
            account_id: id(),
            ..cmd.target.clone()
        },
        DurableAccountRef {
            username: "missing-account".parse().unwrap(),
            ..cmd.target.clone()
        },
        DurableAccountRef {
            authority_id: id(),
            ..cmd.target.clone()
        },
    ] {
        assert!(f
            .source
            .bind_session_account(
                &f.manager_token,
                &SessionBindCommand {
                    target,
                    ..cmd.clone()
                }
            )
            .await
            .is_err());
    }
    assert_eq!(f.count("collaboration_identity_audit").await, 2);
    let receipt = f
        .source
        .bind_session_account(&f.manager_token, &cmd)
        .await
        .unwrap();
    assert_eq!(receipt.entry.actor, Some(f.manager.principal.reference));
    assert_eq!(receipt.entry.after.account_id, f.target.account_id);
    assert_eq!(receipt.entry.after.binding.username, f.target.username);
    assert!(f
        .store
        .legacy_access(scope(), receipt.entry.principal_id)
        .await
        .unwrap()
        .is_none());
    let replay = f
        .source
        .bind_session_account(&f.manager_token, &cmd)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.entry, receipt.entry);
    assert_eq!(f.base.count("users").await, 3);
    assert_eq!(f.base.count("sessions").await, 3);
    let history: String = query("SELECT string_agg(row_to_json(a)::TEXT, '') AS history FROM collaboration_identity_audit a")
        .fetch_one(&f.base.pool).await.unwrap().get("history");
    assert!(
        !history.contains(&f.manager_token)
            && !history.contains(&source_fixture::token_hash(&f.manager_token))
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_policy_revision_replay_and_actor_are_fenced() {
    let f = Fixture::ready().await;
    let target = f.bind_target().await;
    let cmd = policy_command(target.binding.principal_id, true);
    let receipt = f
        .source
        .apply_session_policy_command(&f.manager_token, &cmd)
        .await
        .unwrap();
    assert_eq!(receipt.entry.actor, Some(f.manager.principal.reference));
    let mut revoke = policy_command(target.binding.principal_id, false);
    revoke.expected_revision = Some(receipt.entry.after.revision);
    let revoked = f
        .source
        .apply_session_policy_command(&f.manager_token, &revoke)
        .await
        .unwrap();
    let replay = f
        .source
        .apply_session_policy_command(&f.manager_token, &cmd)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.entry, receipt.entry);
    assert_eq!(
        f.current(target.binding.principal_id).await,
        revoked.entry.after
    );
    assert_eq!(
        f.source
            .apply_session_policy_command(
                &f.manager_token,
                &policy_command(target.binding.principal_id, true)
            )
            .await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::StaleAccess
        ))
    );
    let changed = SessionPolicyCommand {
        desired: policy(target.binding.principal_id, false),
        ..cmd
    };
    assert_eq!(
        f.source
            .apply_session_policy_command(&f.manager_token, &changed)
            .await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::CommandConflict
        ))
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_invalid_expired_revoked_and_recreated_sessions_fail_closed() {
    let f = Fixture::ready().await;
    let cmd = f.bind_command();
    for token in [
        "not-a-token".to_string(),
        Uuid::new_v4().to_string(),
        source_fixture::token_hash(&f.manager_token),
    ] {
        assert_eq!(
            f.source.bind_session_account(&token, &cmd).await,
            Err(SessionCommandError::InvalidSession)
        );
    }
    query("UPDATE sessions SET expires_at = $1 WHERE token = $2")
        .bind(Utc::now() - Duration::seconds(1))
        .bind(source_fixture::token_hash(&f.manager_token))
        .execute(&f.base.pool)
        .await
        .unwrap();
    assert_eq!(
        f.source.bind_session_account(&f.manager_token, &cmd).await,
        Err(SessionCommandError::InvalidSession)
    );
    // Synthetic privileged maintenance only; no live account deletion adapter is exposed.
    f.sql("DELETE FROM users WHERE username = 'source-manager'")
        .await;
    let new = f
        .source
        .register(&f.manager.binding.username, PASSWORD)
        .await
        .unwrap();
    assert_ne!(new.account.account_id, f.manager.account_id);
    let login = f
        .source
        .login(&new.account.username, PASSWORD, &otp(&new.bootstrap.secret))
        .await
        .unwrap();
    assert_eq!(
        f.source.bind_session_account(&login.token, &cmd).await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::AccessDenied
        ))
    );
    assert_eq!(f.count("collaboration_identity_audit").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_roles_unbound_accounts_and_foreign_scopes_cannot_manage() {
    let f = Fixture::ready().await;
    let cmd = f.bind_command();
    for token in [&f.learner_token, &f.target_token] {
        assert_eq!(
            f.source.bind_session_account(token, &cmd).await,
            Err(SessionCommandError::Identity(
                IdentityStoreError::AccessDenied
            ))
        );
    }
    let mut wrong = cmd.clone();
    wrong.workspace.workspace_id = id();
    assert_eq!(
        f.source
            .bind_session_account(&f.manager_token, &wrong)
            .await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::ScopeConflict
        ))
    );
    wrong.workspace.authority_id = id();
    assert_eq!(
        f.source
            .bind_session_account(&f.manager_token, &wrong)
            .await,
        Err(SessionCommandError::Authentication(
            DurableAuthError::SourceMismatch
        ))
    );
    let other = DurableAuthSource::new(f.base.pool.clone(), id());
    assert!(other
        .bind_session_account(&f.manager_token, &cmd)
        .await
        .is_err());
    let mut foreign = policy_command(f.learner.binding.principal_id, true);
    foreign.desired.membership.workspace.workspace_id = id();
    assert!(f
        .source
        .apply_session_policy_command(&f.manager_token, &foreign)
        .await
        .is_err());
    assert_eq!(f.count("collaboration_policy_audit").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_last_manager_and_lost_authority_are_rechecked_on_replay() {
    assert_missing_source_manager_cannot_enable_last_revoke().await;
    let f = Fixture::ready().await;
    let mut self_revoke = policy_command(f.manager.binding.principal_id, false);
    self_revoke.expected_revision = Some(RevisionNumber::try_from(1).unwrap());
    assert_eq!(
        f.source
            .apply_session_policy_command(&f.manager_token, &self_revoke)
            .await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::LastAuthorityManager
        ))
    );
    let mut grant = policy_command(f.learner.binding.principal_id, true);
    grant.expected_revision = Some(RevisionNumber::try_from(1).unwrap());
    f.source
        .apply_session_policy_command(&f.manager_token, &grant)
        .await
        .unwrap();
    f.source
        .apply_session_policy_command(&f.manager_token, &self_revoke)
        .await
        .unwrap();
    assert_eq!(
        f.source
            .apply_session_policy_command(&f.manager_token, &grant)
            .await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::AccessDenied
        ))
    );
    let bind = f.bind_command();
    f.source
        .bind_session_account(&f.learner_token, &bind)
        .await
        .unwrap();
    f.source.logout(&f.learner_token).await.unwrap();
    assert_eq!(
        f.source.bind_session_account(&f.learner_token, &bind).await,
        Err(SessionCommandError::InvalidSession)
    );
    f.cleanup().await;
}

async fn assert_missing_source_manager_cannot_enable_last_revoke() {
    for recreate in [false, true] {
        let f = Fixture::ready().await;
        let mut grant = policy_command(f.learner.binding.principal_id, true);
        grant.expected_revision = Some(RevisionNumber::try_from(1).unwrap());
        f.source
            .apply_session_policy_command(&f.manager_token, &grant)
            .await
            .unwrap();
        // A registry grant whose source account vanished cannot keep a usable manager alive.
        f.sql("DELETE FROM users WHERE username = 'source-learner'")
            .await;
        if recreate {
            let replacement = f
                .source
                .register(&f.learner.binding.username, PASSWORD)
                .await
                .unwrap();
            assert_ne!(replacement.account.account_id, f.learner.account_id);
        }
        let mut revoke = policy_command(f.manager.binding.principal_id, false);
        revoke.expected_revision = Some(RevisionNumber::try_from(1).unwrap());
        assert_eq!(
            f.source
                .apply_session_policy_command(&f.manager_token, &revoke)
                .await,
            Err(SessionCommandError::Identity(
                IdentityStoreError::LastAuthorityManager
            ))
        );
        assert!(
            f.current(f.manager.binding.principal_id)
                .await
                .policy
                .deployment_granted
        );
        assert_eq!(
            f.count("collaboration_policy_audit").await,
            3,
            "failed revoke must roll its receipt back too"
        );
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_reload_and_concurrent_replay_commit_once() {
    let f = Fixture::ready().await;
    let other = f.base.reopen().await;
    let cmd = f.bind_command();
    let (a, b) = tokio::join!(
        f.source.bind_session_account(&f.manager_token, &cmd),
        other.bind_session_account(&f.manager_token, &cmd)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.replayed, b.replayed);
    assert_eq!(a.entry, b.entry);
    assert_eq!(f.count("collaboration_identity_audit").await, 3);
    let cmd = policy_command(a.entry.principal_id, false);
    let (a, b) = tokio::join!(
        f.source
            .apply_session_policy_command(&f.manager_token, &cmd),
        other.apply_session_policy_command(&f.manager_token, &cmd)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.replayed, b.replayed);
    assert_eq!(a.entry, b.entry);
    assert_eq!(f.count("collaboration_policy_audit").await, 3);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_commands_retired_actor_and_missing_policy_target_source_are_denied() {
    let f = Fixture::ready().await;
    let target = f.bind_target().await;
    let mut grant = policy_command(f.learner.binding.principal_id, true);
    grant.expected_revision = Some(RevisionNumber::try_from(1).unwrap());
    f.source
        .apply_session_policy_command(&f.manager_token, &grant)
        .await
        .unwrap();
    f.store
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.learner.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: f.manager.binding.username.clone(),
                account_id: f.manager.account_id,
                expected_principal: f.manager.binding.principal_id,
            },
        })
        .await
        .unwrap();
    assert!(f
        .source
        .validate_session(&f.manager_token)
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        f.source
            .apply_session_policy_command(&f.manager_token, &grant)
            .await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::AccessDenied
        ))
    );
    f.sql("DELETE FROM users WHERE username = 'source-target'")
        .await;
    assert_eq!(
        f.source
            .apply_session_policy_command(
                &f.learner_token,
                &policy_command(target.binding.principal_id, true)
            )
            .await,
        Err(SessionCommandError::AccountMismatch)
    );
    f.cleanup().await;
}
