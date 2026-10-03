use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionPolicyCommand, SessionTaskError, SessionTaskWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, LegacyIdentityAction,
            LegacyIdentityCommand, StoredLegacyAccessPolicy, StoredLegacyIdentity,
        },
        task_store::{TaskDraft, TaskStore, TaskStoreError},
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
mod identity_fixture;
use identity_fixture::scope;
#[path = "session_task/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "session_task/concurrency.rs"]
mod concurrency;
#[path = "session_task/faults.rs"]
mod faults;

#[test]
fn session_task_namespace_is_explicit_and_cannot_inject_search_paths() {
    for name in [
        "",
        "public",
        "pg_catalog",
        "information_schema",
        "a,b",
        "x\"",
        "tasks;select",
        "a.b",
        " leading",
        "Tasks",
        "任务",
    ] {
        assert!(SessionTaskWorkspace::new(name, scope()).is_err(), "{name}");
    }
    assert!(SessionTaskWorkspace::new(&"a".repeat(64), scope()).is_err());
    assert!(SessionTaskWorkspace::new("private_tasks_01", scope()).is_ok());
}

#[tokio::test]
async fn unavailable_session_task_commands_never_fall_back_or_accept_actor_claims() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool, authority());
    let workspace = SessionTaskWorkspace::new("private_tasks", scope()).unwrap();
    assert_eq!(
        source
            .create_session_manual_task("invalid-token", &workspace, id(), "Draft")
            .await,
        Err(SessionTaskError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_eq!(
        source
            .create_session_manual_task(&Uuid::new_v4().to_string(), &workspace, id(), "Draft")
            .await,
        Err(SessionTaskError::Session(
            SessionCommandError::Authentication(DurableAuthError::StorageUnavailable)
        ))
    );
}

#[test]
fn session_task_adapter_cannot_use_detached_authentication_or_live_composition() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(
        root.join("src/services/auth_service/durable_source/task_commands/mod.rs"),
    )
    .unwrap();
    for forbidden in [
        "validate_session(",
        "command_actor(",
        "LegacyRole",
        "std::env",
        "AppState",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
    for helper in [
        "create_in_transaction",
        "get_in_transaction",
        "transition_in_transaction",
        "begin_private_work",
        "context.finish",
    ] {
        assert!(source.contains(helper), "{helper}");
    }
    let guard = std::fs::read_to_string(
        root.join("src/services/auth_service/durable_source/private_work/mod.rs"),
    )
    .unwrap();
    assert!(guard.contains("recheck_command_session"));
    assert!(!guard.contains("validate_session("));
    for file in ["src/state.rs", "src/application.rs"] {
        assert!(!std::fs::read_to_string(root.join(file))
            .unwrap()
            .contains("SessionTaskWorkspace"));
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_owner_is_derived_without_teacher_or_deployment_grants() {
    let f = Fixture::ready().await;
    let first = f.create(&f.auth.learner_token).await;
    assert_eq!(first.owner, f.auth.learner.principal.reference);
    assert_eq!(first.status, TaskStatus::Draft);
    assert_eq!(first.definition, None);
    assert!(first.input_refs.is_empty());
    let ready = f
        .auth
        .source
        .transition_session_manual_task(
            &f.auth.learner_token,
            &f.workspace,
            first.reference.task_id,
            first.revision,
            TaskStatus::Ready,
        )
        .await
        .unwrap();
    assert_eq!(ready.revision, 2.try_into().unwrap());
    assert_eq!(
        f.auth
            .source
            .get_session_manual_task(&f.auth.learner_token, &f.workspace, first.reference.task_id)
            .await
            .unwrap(),
        Some(ready)
    );
    assert_eq!(
        f.auth
            .source
            .get_session_manual_task(&f.auth.manager_token, &f.workspace, first.reference.task_id)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        f.auth
            .source
            .transition_session_manual_task(
                &f.auth.manager_token,
                &f.workspace,
                first.reference.task_id,
                first.revision,
                TaskStatus::Cancelled
            )
            .await,
        Err(SessionTaskError::Task(TaskStoreError::NotFound))
    );
    let namespace: String = query("SELECT current_schema()::text AS name")
        .fetch_one(&f.auth.base.pool)
        .await
        .unwrap()
        .get("name");
    assert_eq!(namespace, f.auth.base.schema);
    assert_eq!(f.count("collaboration_task_outbox").await, 2);
    concurrency::assert_source_pool_restored(&f).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_reject_unbound_missing_membership_and_revoked_sessions() {
    let f = Fixture::ready().await;
    assert!(f
        .auth
        .source
        .create_session_manual_task(&f.auth.target_token, &f.workspace, id(), "Unbound")
        .await
        .is_err());
    f.auth.bind_target().await;
    assert!(f
        .auth
        .source
        .create_session_manual_task(&f.auth.target_token, &f.workspace, id(), "No membership")
        .await
        .is_err());
    f.auth.source.logout(&f.auth.learner_token).await.unwrap();
    assert_eq!(
        f.auth
            .source
            .create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "Revoked")
            .await,
        Err(SessionTaskError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_eq!(f.count("collaboration_tasks").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_reject_domain_or_artifact_work_until_evidence_is_authorized() {
    let f = Fixture::ready().await;
    let owner = f.auth.learner.principal.reference;
    let artifact = ArtifactRef {
        workspace: scope(),
        artifact_id: id(),
        revision_id: id(),
        sha256: "a".repeat(64).parse().unwrap(),
    };
    let stored = f
        .tasks
        .create(
            id(),
            owner,
            TaskDraft::manual("Unresolved input", vec![artifact]).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        f.auth
            .source
            .get_session_manual_task(
                &f.auth.learner_token,
                &f.workspace,
                stored.reference.task_id
            )
            .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    assert_eq!(
        f.auth
            .source
            .transition_session_manual_task(
                &f.auth.learner_token,
                &f.workspace,
                stored.reference.task_id,
                stored.revision,
                TaskStatus::Ready
            )
            .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    use cyanrex_engine::{
        domain_packs::ebpf_teaching::EbpfTeachingPack, services::task_catalog::TaskCatalog,
    };
    let catalog = TaskCatalog::new(EbpfTeachingPack).unwrap();
    let definition = EbpfTeachingPack::definition_ref("01-first-program").unwrap();
    let defined = f
        .tasks
        .create(
            id(),
            owner,
            TaskDraft::from_catalog(&catalog, &definition, "Defined work", vec![]).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        f.auth
            .source
            .get_session_manual_task(
                &f.auth.learner_token,
                &f.workspace,
                defined.reference.task_id
            )
            .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    assert_eq!(
        f.auth
            .source
            .transition_session_manual_task(
                &f.auth.learner_token,
                &f.workspace,
                defined.reference.task_id,
                defined.revision,
                TaskStatus::Ready
            )
            .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    assert_eq!(f.count("collaboration_task_outbox").await, 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_revision_conflicts_and_terminal_states_preserve_events() {
    let f = Fixture::ready().await;
    let first = f.create(&f.auth.learner_token).await;
    let task = first.reference.task_id;
    assert_eq!(
        f.auth
            .source
            .create_session_manual_task(&f.auth.learner_token, &f.workspace, task, "Duplicate")
            .await,
        Err(SessionTaskError::Task(TaskStoreError::Conflict))
    );
    let (a, b) = tokio::join!(
        f.auth.source.transition_session_manual_task(
            &f.auth.learner_token,
            &f.workspace,
            task,
            first.revision,
            TaskStatus::Ready
        ),
        f.auth.source.transition_session_manual_task(
            &f.auth.learner_token,
            &f.workspace,
            task,
            first.revision,
            TaskStatus::Cancelled
        ),
    );
    let second = if let Ok(winner) = a {
        assert_eq!(
            b,
            Err(SessionTaskError::Task(TaskStoreError::StaleRevision))
        );
        winner
    } else {
        assert_eq!(
            a,
            Err(SessionTaskError::Task(TaskStoreError::StaleRevision))
        );
        b.unwrap()
    };
    let terminal = if second.status == TaskStatus::Cancelled {
        second
    } else {
        f.auth
            .source
            .transition_session_manual_task(
                &f.auth.learner_token,
                &f.workspace,
                task,
                second.revision,
                TaskStatus::Cancelled,
            )
            .await
            .unwrap()
    };
    assert_eq!(
        f.auth
            .source
            .transition_session_manual_task(
                &f.auth.learner_token,
                &f.workspace,
                task,
                terminal.revision,
                TaskStatus::Ready
            )
            .await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidTransition))
    );
    assert_eq!(
        f.count("collaboration_task_outbox").await,
        u64::from(terminal.revision) as i64
    );
    let events: Vec<String> = query("SELECT snapshot FROM collaboration_task_outbox")
        .fetch_all(&f.pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get("snapshot"))
        .collect();
    for event in events {
        assert!(!event.contains(&f.auth.learner_token));
        assert!(!event.contains(&source_fixture::token_hash(&f.auth.learner_token)));
    }
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_retired_and_recreated_accounts_cannot_reuse_old_work() {
    let f = Fixture::ready().await;
    let first = f.create(&f.auth.learner_token).await;
    let old = &f.auth.learner;
    f.auth
        .store
        .apply_legacy_identity_command(&LegacyIdentityCommand {
            command_id: id(),
            actor: f.auth.manager.principal.reference,
            workspace: scope(),
            action: LegacyIdentityAction::Retire {
                username: old.binding.username.clone(),
                account_id: old.account_id,
                expected_principal: old.binding.principal_id,
            },
        })
        .await
        .unwrap();
    assert!(f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .is_some());
    assert!(f
        .auth
        .source
        .get_session_manual_task(&f.auth.learner_token, &f.workspace, first.reference.task_id)
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .create_session_manual_task(&f.auth.learner_token, &f.workspace, id(), "Retired")
        .await
        .is_err());
    assert!(f
        .auth
        .source
        .transition_session_manual_task(
            &f.auth.learner_token,
            &f.workspace,
            first.reference.task_id,
            first.revision,
            TaskStatus::Ready
        )
        .await
        .is_err());
    query("DELETE FROM users WHERE username = $1")
        .bind(old.binding.username.as_str())
        .execute(&f.auth.base.pool)
        .await
        .unwrap();
    let account = f
        .auth
        .source
        .register(&old.binding.username, PASSWORD)
        .await
        .unwrap();
    let token = f
        .auth
        .source
        .login(
            &old.binding.username,
            PASSWORD,
            &otp(&account.bootstrap.secret),
        )
        .await
        .unwrap()
        .token;
    let bound = f
        .auth
        .source
        .bind_session_account(
            &f.auth.manager_token,
            &SessionBindCommand {
                command_id: id(),
                workspace: scope(),
                target: account.account,
            },
        )
        .await
        .unwrap()
        .entry
        .after;
    assert_ne!(bound.principal.reference, first.owner);
    let desired = LegacyAccessPolicy {
        membership: Membership {
            workspace: scope(),
            principal_id: bound.binding.principal_id,
            role_refs: vec![],
            status: MembershipStatus::Active,
        },
        deployment_granted: false,
    };
    f.auth
        .source
        .apply_session_policy_command(
            &f.auth.manager_token,
            &SessionPolicyCommand {
                command_id: id(),
                expected_revision: None,
                desired,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        f.auth
            .source
            .get_session_manual_task(&token, &f.workspace, first.reference.task_id)
            .await
            .unwrap(),
        None
    );
    assert!(f
        .auth
        .source
        .get_session_manual_task(&f.auth.learner_token, &f.workspace, first.reference.task_id)
        .await
        .is_err());
    assert_eq!(
        f.tasks.get(first.reference, first.owner).await.unwrap(),
        Some(first)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_tasks_target_namespaces_must_exist_and_cannot_alias_source() {
    let f = Fixture::ready().await;
    for namespace in [&f.auth.base.schema, "nonexistent_session_tasks"] {
        let target = SessionTaskWorkspace::new(namespace, scope()).unwrap();
        assert_eq!(
            f.auth
                .source
                .create_session_manual_task(&f.auth.learner_token, &target, id(), "Wrong namespace")
                .await,
            Err(SessionTaskError::InvalidNamespace)
        );
    }
    let empty = format!("empty_tasks_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {empty}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let target = SessionTaskWorkspace::new(&empty, scope()).unwrap();
    assert!(f
        .auth
        .source
        .create_session_manual_task(&f.auth.learner_token, &target, id(), "No schema adoption")
        .await
        .is_err());
    let count: i64 =
        query("SELECT count(*) AS n FROM pg_class WHERE relnamespace = $1::regnamespace")
            .bind(&empty)
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("n");
    assert_eq!(count, 0);
    query(&format!("DROP SCHEMA {empty}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    assert_eq!(f.count("collaboration_tasks").await, 0);
    f.cleanup().await;
}
