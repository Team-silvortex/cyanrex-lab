use super::*;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, SessionBindCommand, SessionCommandError,
            SessionDeleteAccountCommand, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            LegacyIdentityAction, LegacyIdentityCommand, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
    },
};

#[allow(dead_code)]
#[path = "../durable_collaboration/fixture.rs"]
mod fixture;
use fixture::{scope, Fixture as CollaborationFixture};

async fn secret(f: &CollaborationFixture, name: &LegacyUsername) -> String {
    query("SELECT totp_secret FROM users WHERE username = $1")
        .bind(name.as_str())
        .fetch_one(&f.base.pool)
        .await
        .unwrap()
        .get("totp_secret")
}

async fn registry_state(f: &CollaborationFixture) -> Vec<String> {
    let mut state = Vec::new();
    for table in [
        "collaboration_principals",
        "collaboration_legacy_identities",
        "collaboration_memberships",
        "collaboration_deployment_grants",
        "collaboration_identity_audit",
        "collaboration_policy_audit",
    ] {
        state.push(query(&format!("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text), '[]'::jsonb)::text AS state FROM {table} t"))
            .fetch_one(&f.base.pool).await.unwrap().get("state"));
    }
    state
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_preserves_grants_and_never_reactivates_retired_identity() {
    for kind in ["manager", "learner", "retired"] {
        let f = CollaborationFixture::ready_for_password_change().await;
        let (identity, token) = if kind == "manager" {
            (&f.manager, &f.manager_token)
        } else {
            (&f.learner, &f.learner_token)
        };
        if kind == "retired" {
            f.store
                .apply_legacy_identity_command(&LegacyIdentityCommand {
                    command_id: id(),
                    actor: f.manager.principal.reference,
                    workspace: scope(),
                    action: LegacyIdentityAction::Retire {
                        username: identity.binding.username.clone(),
                        account_id: identity.account_id,
                        expected_principal: identity.binding.principal_id,
                    },
                })
                .await
                .unwrap();
        }
        let before = registry_state(&f).await;
        let secret = secret(&f, &identity.binding.username).await;
        f.source
            .change_session_password(token, PASSWORD, NEW_PASSWORD, &otp(&secret))
            .await
            .unwrap();
        assert_eq!(registry_state(&f).await, before);
        assert_eq!(
            f.source
                .bind_session_account(token, &f.bind_command())
                .await,
            Err(SessionCommandError::InvalidSession)
        );
        let login = f
            .source
            .login(&identity.binding.username, NEW_PASSWORD, &next_otp(&secret))
            .await
            .unwrap();
        assert_eq!(login.session.account.account_id, identity.account_id);
        if kind == "manager" {
            // The sole manager can rotate its credentials without losing the manager grant.
            f.source
                .bind_session_account(&login.token, &f.bind_command())
                .await
                .unwrap();
        } else {
            assert_eq!(
                f.source
                    .bind_session_account(&login.token, &f.bind_command())
                    .await,
                Err(SessionCommandError::Identity(
                    IdentityStoreError::AccessDenied
                ))
            );
            assert_eq!(registry_state(&f).await, before);
        }
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_serializes_session_authorized_binding() {
    for change_first in [false, true] {
        let f = CollaborationFixture::ready_for_password_change().await;
        let secret = secret(&f, &f.manager.binding.username).await;
        let mut blocker = f.base.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70905)")
            .execute(&mut *blocker)
            .await
            .unwrap();
        let (table, timing) = if change_first {
            ("users", "AFTER UPDATE")
        } else {
            ("collaboration_identity_audit", "BEFORE INSERT")
        };
        faults::trigger(
            &f.base,
            table,
            timing,
            "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70905); RETURN NEW;",
        )
        .await;
        let source = f.source.clone();
        let token = f.manager_token.clone();
        let code = otp(&secret);
        let command = f.bind_command();
        let first = tokio::spawn(async move {
            if change_first {
                source
                    .change_session_password(&token, PASSWORD, NEW_PASSWORD, &code)
                    .await
                    .map_err(SessionCommandError::from)
            } else {
                source
                    .bind_session_account(&token, &command)
                    .await
                    .map(|_| ())
            }
        });
        f.base.wait_blocked().await;
        let other = f.base.reopen().await;
        let token = f.manager_token.clone();
        let code = otp(&secret);
        let command = f.bind_command();
        let mut second = tokio::spawn(async move {
            if change_first {
                other
                    .bind_session_account(&token, &command)
                    .await
                    .map(|_| ())
            } else {
                other
                    .change_session_password(&token, PASSWORD, NEW_PASSWORD, &code)
                    .await
                    .map_err(SessionCommandError::from)
            }
        });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut second)
                .await
                .is_err()
        );
        blocker.rollback().await.unwrap();
        first.await.unwrap().unwrap();
        if change_first {
            assert_eq!(
                second.await.unwrap(),
                Err(SessionCommandError::InvalidSession)
            );
        } else {
            second.await.unwrap().unwrap();
        }
        assert_eq!(
            f.count("collaboration_identity_audit").await,
            if change_first { 2 } else { 3 }
        );
        assert_eq!(f.count("collaboration_policy_audit").await, 2);
        assert_eq!(
            f.source.validate_session(&f.manager_token).await.unwrap(),
            None
        );
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_password_change_serializes_administrative_account_deletion() {
    for change_first in [false, true] {
        let f = CollaborationFixture::ready_for_password_change().await;
        let secret = secret(&f, &f.learner.binding.username).await;
        let deletion = SessionDeleteAccountCommand {
            command_id: id(),
            workspace: scope(),
            expected_principal: f.learner.binding.principal_id,
            target: DurableAccountRef {
                authority_id: authority(),
                username: f.learner.binding.username.clone(),
                account_id: f.learner.account_id,
            },
        };
        let mut blocker = f.base.pool.begin().await.unwrap();
        query("SELECT pg_advisory_xact_lock(hashtext(current_schema()), 70906)")
            .execute(&mut *blocker)
            .await
            .unwrap();
        if change_first {
            faults::trigger(
                &f.base,
                "users",
                "AFTER UPDATE",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70906); RETURN NEW;",
            )
            .await;
        } else {
            faults::trigger(
                &f.base,
                "users",
                "AFTER DELETE",
                "PERFORM pg_advisory_xact_lock(hashtext(current_schema()), 70906); RETURN OLD;",
            )
            .await;
        }
        let source = f.source.clone();
        let learner = f.learner_token.clone();
        let manager = f.manager_token.clone();
        let code = otp(&secret);
        let command = deletion.clone();
        let first = tokio::spawn(async move {
            if change_first {
                source
                    .change_session_password(&learner, PASSWORD, NEW_PASSWORD, &code)
                    .await
                    .map_err(SessionCommandError::from)
            } else {
                source
                    .delete_session_account(&manager, &command)
                    .await
                    .map(|_| ())
            }
        });
        f.base.wait_blocked().await;
        let other = f.base.reopen().await;
        let learner = f.learner_token.clone();
        let manager = f.manager_token.clone();
        let code = otp(&secret);
        let mut second = tokio::spawn(async move {
            if change_first {
                other
                    .delete_session_account(&manager, &deletion)
                    .await
                    .map(|_| ())
            } else {
                other
                    .change_session_password(&learner, PASSWORD, NEW_PASSWORD, &code)
                    .await
                    .map_err(SessionCommandError::from)
            }
        });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut second)
                .await
                .is_err()
        );
        blocker.rollback().await.unwrap();
        first.await.unwrap().unwrap();
        if change_first {
            second.await.unwrap().unwrap();
        } else {
            assert_eq!(
                second.await.unwrap(),
                Err(SessionCommandError::Authentication(
                    DurableAuthError::InvalidCredentials
                ))
            );
        }
        assert_eq!(f.base.count("users").await, 2);
        assert_eq!(f.base.count("sessions").await, 2);
        assert_eq!(f.count("collaboration_identity_audit").await, 3);
        f.cleanup().await;
    }
}
