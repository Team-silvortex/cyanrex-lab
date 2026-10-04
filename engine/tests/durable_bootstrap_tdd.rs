use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            BootstrapError, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            LegacyPolicyResource,
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
use source_fixture::*;
#[path = "durable_bootstrap/concurrency.rs"]
mod concurrency;
#[path = "durable_bootstrap/faults.rs"]
mod faults;

fn scope() -> WorkspaceRef {
    WorkspaceRef {
        authority_id: authority(),
        workspace_id: "60a9845d-0687-4f19-a4f5-2ebf64bed4ba".parse().unwrap(),
    }
}

async fn fixture_guard() -> sqlx_core::transaction::Transaction<'static, sqlx_postgres::Postgres> {
    // PostgreSQL event-trigger catalogs are database-global even when callbacks filter schemas.
    // Serialize whole test cases (also across test processes), not the competing operations inside
    // each case, so removing a hook cannot race another case's cached DDL callback list.
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(
            &std::env::var("CYANREX_TEST_DATABASE_URL").expect("disposable test database required"),
        )
        .await
        .unwrap();
    let mut guard = pool.begin().await.unwrap();
    query("SELECT pg_advisory_xact_lock(hashtext(current_database()), hashtext('cyanrex.test.bootstrap.event-hooks'))")
        .execute(&mut *guard).await.unwrap();
    guard
}

async fn objects(f: &Fixture) -> i64 {
    query(
        "SELECT (SELECT count(*) FROM pg_class WHERE relnamespace = $1::regnamespace)
        + (SELECT count(*) FROM pg_proc WHERE pronamespace = $1::regnamespace)
        + (SELECT count(*) FROM pg_type WHERE typnamespace = $1::regnamespace) AS count",
    )
    .bind(&f.schema)
    .fetch_one(&f.admin)
    .await
    .unwrap()
    .get("count")
}

async fn isolated_pool(f: &Fixture, path: &str) -> PgPool {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(f.pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    query("SELECT set_config('search_path', $1, false)")
        .bind(path)
        .execute(&pool)
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn closed_bootstrap_never_creates_an_in_memory_manager() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1:1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool, authority());
    assert!(matches!(
        source
            .bootstrap_empty_authority(scope(), &username(), PASSWORD)
            .await,
        Err(BootstrapError::Authentication(
            DurableAuthError::StorageUnavailable
        ))
    ));
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_bootstrap_commits_source_owner_grant_and_audit_together() {
    let _guard = fixture_guard().await;
    let f = Fixture::new().await;
    let created = f
        .source
        .bootstrap_empty_authority(scope(), &username(), PASSWORD)
        .await
        .unwrap();
    assert_eq!(created.workspace.reference, scope());
    assert_eq!(created.registration.account.username, username());
    assert_eq!(
        created.identity.account_id,
        created.registration.account.account_id
    );
    assert_eq!(created.identity.principal.kind, PrincipalKind::Human);
    assert!(created.policy.policy.deployment_granted);
    assert_eq!(
        created
            .policy
            .policy
            .membership
            .role_refs
            .iter()
            .map(QualifiedName::as_str)
            .collect::<Vec<_>>(),
        vec!["cyanrex.teaching.teacher", "cyanrex.workspace.owner"]
    );
    assert_eq!(f.count("users").await, 1);
    assert_eq!(
        f.count("sessions").await,
        0,
        "bootstrap does not bypass TOTP by issuing a token"
    );
    let store = CollaborationIdentityStore::new(f.pool.clone());
    assert_eq!(
        store
            .lookup_legacy_account(
                scope(),
                &username(),
                created.registration.account.account_id
            )
            .await
            .unwrap(),
        Some(created.identity.clone())
    );
    assert_eq!(
        store
            .legacy_access(scope(), created.identity.binding.principal_id)
            .await
            .unwrap(),
        Some(created.policy)
    );
    let reopened = f.reopen().await;
    let login = reopened
        .login(
            &username(),
            PASSWORD,
            &otp(&created.registration.bootstrap.secret),
        )
        .await
        .unwrap();
    assert_eq!(login.session.account, created.registration.account);
    for table in [
        "collaboration_identity_schema",
        "collaboration_access_schema",
    ] {
        assert_eq!(
            query(&format!("SELECT version FROM {table}"))
                .fetch_one(&f.pool)
                .await
                .unwrap()
                .get::<i32, _>("version"),
            2
        );
    }
    for table in ["collaboration_identity_audit", "collaboration_policy_audit"] {
        let row = query(&format!(
            "SELECT kind, actor_principal_id, command_id FROM {table}"
        ))
        .fetch_one(&f.pool)
        .await
        .unwrap();
        assert_eq!(row.get::<String, _>("kind"), "baseline");
        assert_eq!(row.get::<Option<Uuid>, _>("actor_principal_id"), None);
        assert_eq!(row.get::<Option<Uuid>, _>("command_id"), None);
    }
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_bootstrap_repeated_or_changed_requests_never_regrant_or_reveal_secrets() {
    let _guard = fixture_guard().await;
    let f = Fixture::new().await;
    let created = f
        .source
        .bootstrap_empty_authority(scope(), &username(), PASSWORD)
        .await
        .unwrap();
    let before = objects(&f).await;
    let peer = f.reopen().await;
    for (scope, name) in [
        (scope(), username()),
        (
            WorkspaceRef {
                workspace_id: id(),
                ..scope()
            },
            username(),
        ),
        (scope(), "second-owner".parse().unwrap()),
    ] {
        assert!(matches!(
            peer.bootstrap_empty_authority(scope, &name, "different-password")
                .await,
            Err(BootstrapError::NotEmpty)
        ));
    }
    assert_eq!(objects(&f).await, before);
    assert_eq!(f.count("users").await, 1);
    // Even loss of the source account is NOT permission to create an emergency manager.
    f.sql("DELETE FROM users").await;
    assert!(matches!(
        peer.bootstrap_empty_authority(scope(), &username(), PASSWORD)
            .await,
        Err(BootstrapError::NotEmpty)
    ));
    assert_eq!(f.count("users").await, 0);
    let store = CollaborationIdentityStore::new(f.pool.clone());
    assert_eq!(
        store
            .lookup_legacy_account(
                scope(),
                &username(),
                created.registration.account.account_id
            )
            .await
            .unwrap(),
        Some(created.identity)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_bootstrap_rejects_existing_data_schema_objects_and_partial_installation() {
    let _guard = fixture_guard().await;
    for prior in [
        "legacy",
        "source",
        "identity",
        "unrelated",
        "function",
        "type",
        "view",
    ] {
        let f = Fixture::new().await;
        match prior {
            "legacy" => {
                for statement in include_str!("../migrations/0001_auth_users_sessions.sql")
                    .split(';')
                    .filter(|s| !s.trim().is_empty())
                {
                    f.sql(statement).await;
                }
                f.sql("INSERT INTO users (username, password_salt, password_hash, totp_secret) VALUES ('existing-user', 'synthetic', 'synthetic', 'synthetic')").await;
            }
            "source" => {
                f.source.install_empty_schema().await.unwrap();
            }
            "identity" => {
                CollaborationIdentityStore::new(f.pool.clone())
                    .install_schema()
                    .await
                    .unwrap();
            }
            "unrelated" => {
                f.sql("CREATE TABLE sentinel (value text)").await;
                f.sql("INSERT INTO sentinel VALUES ('keep')").await;
            }
            "function" => {
                f.sql("CREATE FUNCTION sentinel() RETURNS integer LANGUAGE sql AS 'SELECT 1'")
                    .await;
            }
            "type" => {
                f.sql("CREATE TYPE sentinel AS ENUM ('keep')").await;
            }
            "view" => {
                f.sql("CREATE VIEW sentinel AS SELECT 'keep'::text AS value")
                    .await;
            }
            _ => unreachable!(),
        }
        let before = objects(&f).await;
        assert!(
            matches!(
                f.source
                    .bootstrap_empty_authority(scope(), &username(), PASSWORD)
                    .await,
                Err(BootstrapError::NotEmpty)
            ),
            "{prior}"
        );
        assert_eq!(objects(&f).await, before);
        if prior == "legacy" {
            assert_eq!(f.count("users").await, 1);
        }
        if matches!(prior, "unrelated" | "view") {
            assert_eq!(
                query("SELECT value FROM sentinel")
                    .fetch_one(&f.pool)
                    .await
                    .unwrap()
                    .get::<String, _>("value"),
                "keep"
            );
        }
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_bootstrap_rejects_bad_input_scope_and_ambiguous_search_path() {
    let _guard = fixture_guard().await;
    let f = Fixture::new().await;
    for password in ["short".to_string(), "x".repeat(4097)] {
        assert!(matches!(
            f.source
                .bootstrap_empty_authority(scope(), &username(), &password)
                .await,
            Err(BootstrapError::Authentication(
                DurableAuthError::InvalidInput
            ))
        ));
    }
    assert!(matches!(
        f.source
            .bootstrap_empty_authority(
                WorkspaceRef {
                    authority_id: id(),
                    ..scope()
                },
                &username(),
                PASSWORD
            )
            .await,
        Err(BootstrapError::Authentication(
            DurableAuthError::SourceMismatch
        ))
    ));
    for path in [
        format!("{}, public", f.schema),
        "pg_catalog".to_string(),
        "nonexistent_bootstrap_namespace".to_string(),
    ] {
        let pool = isolated_pool(&f, &path).await;
        let source = DurableAuthSource::new(pool.clone(), authority());
        assert!(
            matches!(
                source
                    .bootstrap_empty_authority(scope(), &username(), PASSWORD)
                    .await,
                Err(BootstrapError::InvalidNamespace)
            ),
            "{path}"
        );
        pool.close().await;
    }
    let pool = isolated_pool(&f, &f.schema).await;
    query("CREATE TEMP TABLE users (value text)")
        .execute(&pool)
        .await
        .unwrap();
    let source = DurableAuthSource::new(pool.clone(), authority());
    assert!(matches!(
        source
            .bootstrap_empty_authority(scope(), &username(), PASSWORD)
            .await,
        Err(BootstrapError::InvalidNamespace)
    ));
    pool.close().await;
    assert_eq!(objects(&f).await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_bootstrap_pins_each_namespace_without_cross_authority_access() {
    let _guard = fixture_guard().await;
    let a = Fixture::new().await;
    let b = Fixture::new().await;
    let other_scope = WorkspaceRef {
        authority_id: id(),
        workspace_id: id(),
    };
    let other = DurableAuthSource::new(b.pool.clone(), other_scope.authority_id);
    let name = username();
    let (one, two) = tokio::join!(
        a.source.bootstrap_empty_authority(scope(), &name, PASSWORD),
        other.bootstrap_empty_authority(other_scope, &name, PASSWORD)
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_ne!(
        one.registration.account.account_id,
        two.registration.account.account_id
    );
    assert_ne!(
        one.identity.binding.principal_id,
        two.identity.binding.principal_id
    );
    let login = a
        .source
        .login(
            &username(),
            PASSWORD,
            &otp(&one.registration.bootstrap.secret),
        )
        .await
        .unwrap();
    assert_eq!(other.validate_session(&login.token).await.unwrap(), None);
    assert_eq!(
        b.source.validate_session(&login.token).await,
        Err(DurableAuthError::SourceMismatch)
    );
    a.cleanup().await;
    b.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_bootstrap_manager_uses_audited_commands_without_granting_new_accounts() {
    let _guard = fixture_guard().await;
    let f = Fixture::new().await;
    let created = f
        .source
        .bootstrap_empty_authority(scope(), &username(), PASSWORD)
        .await
        .unwrap();
    let secret = created.registration.bootstrap.secret;
    // This graph/rotation test seeds its authorizing Session; first-login behavior is covered above.
    let login = f.seed_session(&created.registration.account).await;
    let member = f
        .source
        .register(&"ordinary-member".parse().unwrap(), PASSWORD)
        .await
        .unwrap();
    let member_login = f
        .source
        .login(
            &member.account.username,
            PASSWORD,
            &otp(&member.bootstrap.secret),
        )
        .await
        .unwrap();
    let cmd = SessionBindCommand {
        command_id: id(),
        workspace: scope(),
        target: member.account.clone(),
    };
    assert_eq!(
        f.source
            .bind_session_account(&member_login.token, &cmd)
            .await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::AccessDenied
        ))
    );
    let identity = f
        .source
        .bind_session_account(&login.token, &cmd)
        .await
        .unwrap()
        .entry
        .after;
    let store = CollaborationIdentityStore::new(f.pool.clone());
    assert_eq!(
        store
            .legacy_access(scope(), identity.binding.principal_id)
            .await
            .unwrap(),
        None
    );
    for (action, resource, allowed) in [
        (
            LegacyAction::ManageDeployment,
            LegacyPolicyResource::Deployment {
                authority_id: authority(),
            },
            true,
        ),
        (
            LegacyAction::ManageDeployment,
            LegacyPolicyResource::Deployment { authority_id: id() },
            false,
        ),
        (
            LegacyAction::ReadPrivateArtifact,
            LegacyPolicyResource::PrivateArtifact {
                workspace: scope(),
                owner_id: identity.binding.principal_id,
            },
            false,
        ),
    ] {
        assert_eq!(
            store
                .preview_legacy_access(
                    scope(),
                    created.identity.principal.reference,
                    action,
                    &resource
                )
                .await
                .unwrap(),
            allowed
        );
    }
    let revoke = SessionPolicyCommand {
        command_id: id(),
        expected_revision: Some(created.policy.revision),
        desired: LegacyAccessPolicy {
            deployment_granted: false,
            ..created.policy.policy
        },
    };
    assert_eq!(
        f.source
            .apply_session_policy_command(&login.token, &revoke)
            .await,
        Err(SessionCommandError::Identity(
            IdentityStoreError::LastAuthorityManager
        ))
    );
    assert_eq!(
        store
            .bind_legacy_account(scope(), &member.account.username, member.account.account_id)
            .await,
        Err(IdentityStoreError::IdentityAuditContextRequired)
    );
    f.source
        .change_session_password(
            &login.token,
            PASSWORD,
            "replacement-password",
            &otp(&secret),
        )
        .await
        .unwrap();
    assert_eq!(
        f.source.bind_session_account(&login.token, &cmd).await,
        Err(SessionCommandError::InvalidSession)
    );
    assert_eq!(
        f.source
            .validate_session(&member_login.token)
            .await
            .unwrap(),
        Some(member_login.session)
    );
    let new_login = f
        .source
        .login(&username(), "replacement-password", &next_otp(&secret))
        .await
        .unwrap();
    assert!(
        f.source
            .bind_session_account(&new_login.token, &cmd)
            .await
            .unwrap()
            .replayed
    );
    f.cleanup().await;
}
