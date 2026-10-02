use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionDeleteAccountCommand, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            LegacyIdentityAction, LegacyIdentityCommand, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        legacy_workspace::LegacyAction,
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::{authority, id, otp, PASSWORD};
#[allow(dead_code)]
#[path = "durable_collaboration/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "durable_account_deletion/concurrency.rs"]
mod concurrency;
#[path = "durable_account_deletion/faults.rs"]
mod faults;

fn deletion(identity: &StoredLegacyIdentity) -> SessionDeleteAccountCommand {
    SessionDeleteAccountCommand {
        command_id: id(),
        workspace: scope(),
        target: DurableAccountRef {
            authority_id: authority(),
            username: identity.binding.username.clone(),
            account_id: identity.account_id,
        },
        expected_principal: identity.binding.principal_id,
    }
}

async fn assert_unchanged(f: &Fixture, identity_audits: i64) {
    assert_eq!(f.base.count("users").await, 3);
    assert_eq!(f.base.count("sessions").await, 3);
    assert_eq!(
        f.count("collaboration_identity_audit").await,
        identity_audits
    );
    assert_eq!(
        f.store
            .lookup_legacy_account(scope(), &f.learner.binding.username, f.learner.account_id)
            .await
            .unwrap(),
        Some(f.learner.clone())
    );
}

async fn grant_learner(f: &Fixture) {
    let mut cmd = policy_command(f.learner.binding.principal_id, true);
    cmd.expected_revision = Some(RevisionNumber::try_from(1).unwrap());
    f.source
        .apply_session_policy_command(&f.manager_token, &cmd)
        .await
        .unwrap();
}

#[tokio::test]
async fn closed_account_deletion_never_falls_back() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool, authority());
    let cmd = SessionDeleteAccountCommand {
        command_id: id(),
        workspace: scope(),
        expected_principal: id(),
        target: DurableAccountRef {
            authority_id: authority(),
            username: "synthetic-user".parse().unwrap(),
            account_id: id(),
        },
    };
    assert_eq!(
        source
            .delete_session_account(&Uuid::new_v4().to_string(), &cmd)
            .await,
        Err(SessionCommandError::Authentication(
            DurableAuthError::StorageUnavailable
        ))
    );
    assert_eq!(
        source.delete_session_account("invalid", &cmd).await,
        Err(SessionCommandError::InvalidSession)
    );
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_atomically_retires_revokes_all_sessions_and_preserves_history() {
    let f = Fixture::ready().await;
    grant_learner(&f).await;
    let policy_before = f.current(f.learner.binding.principal_id).await;
    // Include an expired second session: deletion must revoke every session, not just active ones.
    query("INSERT INTO sessions (token, username, account_id, expires_at) VALUES ($1, $2, $3, clock_timestamp() - INTERVAL '1 day')")
        .bind(source_fixture::token_hash(&Uuid::new_v4().to_string()))
        .bind(f.learner.binding.username.as_str()).bind(f.learner.account_id.as_uuid())
        .execute(&f.base.pool).await.unwrap();
    let cmd = deletion(&f.learner);
    let receipt = f
        .source
        .delete_session_account(&f.manager_token, &cmd)
        .await
        .unwrap();
    assert!(!receipt.replayed);
    assert_eq!(receipt.entry.actor, Some(f.manager.principal.reference));
    assert_eq!(receipt.entry.before, Some(f.learner.clone()));
    assert!(receipt.entry.after.retired_at.is_some());
    assert_eq!(
        receipt.entry.after.principal.status,
        PrincipalStatus::Disabled
    );
    assert_eq!(f.base.count("users").await, 2);
    assert_eq!(f.base.count("sessions").await, 2);
    assert_eq!(
        f.current(f.learner.binding.principal_id).await,
        policy_before
    );
    assert_eq!(f.count("collaboration_policy_audit").await, 3);
    let other = f.base.reopen().await;
    assert!(other
        .validate_session(&f.learner_token)
        .await
        .unwrap()
        .is_none());
    assert!(other
        .validate_session(&f.manager_token)
        .await
        .unwrap()
        .is_some());
    assert!(!f.store.preview_legacy_access(scope(), f.learner.principal.reference,
        LegacyAction::ManageDeployment,
        &cyanrex_engine::services::collaboration_identity_store::LegacyPolicyResource::Deployment { authority_id: authority() })
        .await.unwrap());
    assert_eq!(
        f.store
            .lookup_legacy_account(scope(), &cmd.target.username, cmd.target.account_id)
            .await
            .unwrap(),
        Some(receipt.entry.after)
    );
    let history: String = query("SELECT string_agg(row_to_json(a)::TEXT, '') AS history FROM collaboration_identity_audit a")
        .fetch_one(&f.base.pool).await.unwrap().get("history");
    for token in [&f.manager_token, &f.learner_token] {
        assert!(!history.contains(token) && !history.contains(&source_fixture::token_hash(token)));
    }
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_exact_coordinates_actor_and_scope_are_required() {
    let f = Fixture::ready().await;
    let cmd = deletion(&f.learner);
    for token in [&f.learner_token, &f.target_token] {
        assert_eq!(
            f.source.delete_session_account(token, &cmd).await,
            Err(SessionCommandError::Identity(
                IdentityStoreError::AccessDenied
            ))
        );
    }
    for wrong in [
        SessionDeleteAccountCommand {
            expected_principal: id(),
            ..cmd.clone()
        },
        SessionDeleteAccountCommand {
            target: DurableAccountRef {
                account_id: id(),
                ..cmd.target.clone()
            },
            ..cmd.clone()
        },
        SessionDeleteAccountCommand {
            target: DurableAccountRef {
                username: "missing-account".parse().unwrap(),
                ..cmd.target.clone()
            },
            ..cmd.clone()
        },
        SessionDeleteAccountCommand {
            target: DurableAccountRef {
                authority_id: id(),
                ..cmd.target.clone()
            },
            ..cmd.clone()
        },
        SessionDeleteAccountCommand {
            workspace: WorkspaceRef {
                workspace_id: id(),
                ..scope()
            },
            ..cmd.clone()
        },
        SessionDeleteAccountCommand {
            workspace: WorkspaceRef {
                authority_id: id(),
                ..scope()
            },
            ..cmd.clone()
        },
        SessionDeleteAccountCommand {
            target: f.target.clone(),
            ..cmd.clone()
        },
    ] {
        assert!(f
            .source
            .delete_session_account(&f.manager_token, &wrong)
            .await
            .is_err());
    }
    assert_unchanged(&f, 2).await;
    f.source.logout(&f.manager_token).await.unwrap();
    assert_eq!(
        f.source
            .delete_session_account(&f.manager_token, &cmd)
            .await,
        Err(SessionCommandError::InvalidSession)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_self_and_last_manager_are_out_of_scope() {
    let f = Fixture::ready().await;
    for extra_manager in [false, true] {
        if extra_manager {
            grant_learner(&f).await;
        }
        assert_eq!(
            f.source
                .delete_session_account(&f.manager_token, &deletion(&f.manager))
                .await,
            Err(SessionCommandError::SelfDeletionUnsupported)
        );
        assert_unchanged(&f, 2).await;
    }
    // A recreated same-name account cannot use its old manager binding.
    f.sql("DELETE FROM users WHERE username = 'source-manager'")
        .await;
    let account = f
        .source
        .register(&f.manager.binding.username, PASSWORD)
        .await
        .unwrap();
    let login = f
        .source
        .login(
            &account.account.username,
            PASSWORD,
            &otp(&account.bootstrap.secret),
        )
        .await
        .unwrap();
    assert_eq!(
        f.source
            .delete_session_account(&login.token, &deletion(&f.learner))
            .await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::AccessDenied
        ))
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_replay_and_recreation_never_delete_a_replacement() {
    let f = Fixture::ready().await;
    let cmd = deletion(&f.learner);
    f.source
        .delete_session_account(&f.manager_token, &cmd)
        .await
        .unwrap();
    assert_eq!(
        f.source
            .delete_session_account(&f.manager_token, &cmd)
            .await,
        Err(SessionCommandError::AccountMismatch)
    );
    let replacement = f
        .source
        .register(&cmd.target.username, PASSWORD)
        .await
        .unwrap();
    let replacement_binding = f
        .source
        .bind_session_account(
            &f.manager_token,
            &SessionBindCommand {
                command_id: id(),
                workspace: scope(),
                target: replacement.account.clone(),
            },
        )
        .await
        .unwrap()
        .entry
        .after;
    assert_ne!(replacement_binding.account_id, cmd.target.account_id);
    assert_ne!(
        replacement_binding.binding.principal_id,
        cmd.expected_principal
    );
    assert!(f
        .store
        .legacy_access(scope(), replacement_binding.binding.principal_id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        f.source
            .delete_session_account(&f.manager_token, &cmd)
            .await,
        Err(SessionCommandError::AccountMismatch)
    );
    // Reusing the ID with the replacement's coordinates is a conflict, never a second deletion.
    let changed = SessionDeleteAccountCommand {
        command_id: cmd.command_id,
        ..deletion(&replacement_binding)
    };
    assert_eq!(
        f.source
            .delete_session_account(&f.manager_token, &changed)
            .await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::CommandConflict
        ))
    );
    assert_eq!(f.base.count("users").await, 3);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_standalone_retirement_is_not_a_delete_receipt() {
    let f = Fixture::ready().await;
    let cmd = deletion(&f.learner);
    f.store
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: cmd.command_id,
            actor: f.manager.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: cmd.target.username.clone(),
                account_id: cmd.target.account_id,
                expected_principal: cmd.expected_principal,
            },
        })
        .await
        .unwrap();
    for command_id in [cmd.command_id, id()] {
        assert_eq!(
            f.source
                .delete_session_account(
                    &f.manager_token,
                    &SessionDeleteAccountCommand {
                        command_id,
                        ..cmd.clone()
                    }
                )
                .await,
            Err(SessionCommandError::Identity(
                IdentityStoreError::IdentityRetired
            ))
        );
    }
    assert_eq!(f.base.count("users").await, 3);
    assert_eq!(f.base.count("sessions").await, 3);
    assert_eq!(f.count("collaboration_identity_audit").await, 3);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_deletion_requires_explicit_schemas_and_current_audit() {
    let f = Fixture::seeded().await;
    let cmd = deletion(&f.learner);
    assert!(f
        .source
        .delete_session_account(&f.manager_token, &cmd)
        .await
        .is_err());
    f.store.upgrade_policy_audit_schema().await.unwrap();
    assert!(f
        .source
        .delete_session_account(&f.manager_token, &cmd)
        .await
        .is_err());
    f.store.upgrade_identity_audit_schema().await.unwrap();
    for table in [
        "collaboration_auth_source_schema",
        "collaboration_identity_audit",
        "collaboration_policy_audit",
    ] {
        f.sql(&format!("ALTER TABLE {table} RENAME TO missing_table"))
            .await;
        assert!(f
            .source
            .delete_session_account(&f.manager_token, &cmd)
            .await
            .is_err());
        f.sql(&format!("ALTER TABLE missing_table RENAME TO {table}"))
            .await;
        assert_unchanged(&f, 2).await;
    }
    f.cleanup().await;
}
