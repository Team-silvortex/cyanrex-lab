#![cfg(unix)]
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore, ArtifactStoreError},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionArtifactWorkspace,
            SessionBindCommand, SessionCatalogTaskWorkspace, SessionCommandError,
            SessionPolicyCommand, SessionTaskError, SessionTaskInputs, SessionTaskInputsWorkspace,
            SessionTaskWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, LegacyIdentityAction,
            LegacyIdentityCommand, StoredLegacyAccessPolicy, StoredLegacyIdentity,
        },
        task_catalog::{TaskCatalog, TaskCatalogError, TaskProvider},
        task_store::{TaskDraft, TaskStore, TaskStoreError},
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{fs, path::PathBuf};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::{authority, id, otp, PASSWORD};
#[allow(dead_code)]
#[path = "durable_collaboration/fixture.rs"]
mod identity_fixture;
use identity_fixture::scope;
#[path = "session_catalog_task/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "session_catalog_task/concurrency.rs"]
mod concurrency;
#[path = "session_catalog_task/faults.rs"]
mod faults;
#[path = "session_catalog_task/lifecycle.rs"]
mod lifecycle;

#[tokio::test]
async fn session_catalog_task_configuration_requires_distinct_namespaces_and_matching_scope() {
    let source = unavailable_source().await;
    let directory = private_directory();
    let artifacts = source
        .open_artifact_workspace("private_artifacts", scope(), &directory)
        .unwrap();
    let catalog = catalog(vec![definition(1)]);
    assert!(SessionCatalogTaskWorkspace::new(
        SessionTaskWorkspace::new("private_tasks", scope()).unwrap(),
        artifacts.clone(),
        &catalog
    )
    .is_ok());
    assert!(matches!(
        SessionCatalogTaskWorkspace::new(
            SessionTaskWorkspace::new("private_artifacts", scope()).unwrap(),
            artifacts.clone(),
            &catalog
        ),
        Err(SessionTaskError::InvalidNamespace)
    ));
    for changed in [
        WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        WorkspaceRef {
            authority_id: id(),
            ..scope()
        },
    ] {
        assert!(matches!(
            SessionCatalogTaskWorkspace::new(
                SessionTaskWorkspace::new("private_tasks", changed).unwrap(),
                artifacts.clone(),
                &catalog
            ),
            Err(SessionTaskError::Task(TaskStoreError::ScopeMismatch))
        ));
    }
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop(artifacts);
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn session_catalog_tasks_fail_closed_and_reject_unknown_or_invalid_inputs_before_database() {
    let source = unavailable_source().await;
    let directory = private_directory();
    let workspace = offline_workspace(&source, &directory);
    let token = Uuid::new_v4().to_string();
    assert_eq!(
        source
            .create_session_catalog_task("invalid", &workspace, id(), &reference(), "Draft", vec![])
            .await,
        Err(SessionTaskError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    let unavailable = SessionTaskError::Session(SessionCommandError::Authentication(
        DurableAuthError::StorageUnavailable,
    ));
    assert_eq!(
        source
            .create_session_catalog_task(&token, &workspace, id(), &reference(), "Draft", vec![])
            .await,
        Err(unavailable)
    );
    assert_eq!(
        source
            .get_session_catalog_task(&token, &workspace, id())
            .await,
        Err(unavailable)
    );
    assert_eq!(
        source
            .transition_session_catalog_task(
                &token,
                &workspace,
                id(),
                1.try_into().unwrap(),
                TaskStatus::Ready
            )
            .await,
        Err(unavailable)
    );
    let unknown = definition(2).reference;
    assert_eq!(
        source
            .create_session_catalog_task(&token, &workspace, id(), &unknown, "Draft", vec![])
            .await,
        Err(SessionTaskError::Task(TaskStoreError::UnknownDefinition))
    );
    let input = ArtifactRef {
        workspace: scope(),
        artifact_id: id(),
        revision_id: id(),
        sha256: "a".repeat(64).parse().unwrap(),
    };
    let conflict = ArtifactRef {
        sha256: "b".repeat(64).parse().unwrap(),
        ..input.clone()
    };
    for inputs in [
        vec![input.clone(); 33],
        vec![input.clone(), input.clone()],
        vec![input, conflict],
    ] {
        assert_eq!(
            source
                .create_session_catalog_task(
                    &token,
                    &workspace,
                    id(),
                    &reference(),
                    "Draft",
                    inputs
                )
                .await,
            Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
        );
    }
    assert_eq!(
        source
            .create_session_catalog_task(&token, &workspace, id(), &reference(), " ", vec![])
            .await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
    );
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop(workspace);
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn session_catalog_workspace_owns_only_metadata_after_non_send_provider_is_dropped() {
    let source = unavailable_source().await;
    let directory = private_directory();
    let (catalog, probe) = tracked_catalog(vec![definition(1)]);
    assert_eq!(probe.packages.get(), 1);
    assert_eq!(probe.definitions.get(), 1);
    let workspace = SessionCatalogTaskWorkspace::new(
        SessionTaskWorkspace::new("private_tasks", scope()).unwrap(),
        source
            .open_artifact_workspace("private_artifacts", scope(), &directory)
            .unwrap(),
        &catalog,
    )
    .unwrap();
    fn send_sync(_: &(impl Send + Sync)) {}
    send_sync(&workspace); // The provider contains Rc<Cell<_>>, which cannot cross this boundary.
    drop(catalog);
    assert_eq!(probe.drops.get(), 1);
    assert_eq!(probe.packages.get(), 1);
    assert_eq!(probe.definitions.get(), 1);
    assert_eq!(
        source
            .create_session_catalog_task("invalid", &workspace, id(), &reference(), "Draft", vec![])
            .await,
        Err(SessionTaskError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    assert_eq!(probe.assessments.get(), 0);
    drop(workspace);
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_keep_non_teaching_definitions_and_exact_ordered_inputs() {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let empty = f.create(token, vec![]).await;
    assert_eq!(empty.owner, f.auth.learner.principal.reference);
    assert_eq!(empty.definition, Some(definition(1)));
    let observed = f
        .get(token, empty.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observed.task, empty);
    assert!(observed.inputs.is_empty());
    assert_eq!(f.files(), 0);
    // Empty inputs do not bypass the configured Artifact namespace and metadata checks.
    f.sql_artifact(
        "ALTER TABLE collaboration_artifact_schema RENAME TO unavailable_catalog_artifact_schema",
    )
    .await;
    assert!(f.request(token, id(), vec![]).await.is_err());
    assert!(f.get(token, empty.reference.task_id).await.is_err());
    assert!(f
        .transition(token, &empty, TaskStatus::Ready)
        .await
        .is_err());
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    f.sql_artifact(
        "ALTER TABLE unavailable_catalog_artifact_schema RENAME TO collaboration_artifact_schema",
    )
    .await;
    query("UPDATE collaboration_artifact_schema SET workspace_id = $1")
        .bind(id::<WorkspaceId>().as_uuid())
        .execute(&f.artifact_pool)
        .await
        .unwrap();
    assert!(f.request(token, id(), vec![]).await.is_err());
    assert!(f.get(token, empty.reference.task_id).await.is_err());
    query("UPDATE collaboration_artifact_schema SET workspace_id = $1")
        .bind(scope().workspace_id.as_uuid())
        .execute(&f.artifact_pool)
        .await
        .unwrap();
    let first = f.create_artifact(token, b"Original text").await;
    let other = f.create_artifact(token, b"Other input").await;
    let second = f
        .auth
        .source
        .revise_session_artifact(
            token,
            &f.artifact_workspace,
            &first.reference,
            id(),
            draft(b"Revised text"),
        )
        .await
        .unwrap();
    let inputs = vec![other.reference, first.reference, second.reference.clone()];
    let task = f.create(token, inputs.clone()).await;
    f.auth
        .source
        .revise_session_artifact(
            token,
            &f.artifact_workspace,
            &second.reference,
            id(),
            draft(b"Latest is not selected"),
        )
        .await
        .unwrap();
    let loaded = f.get(token, task.reference.task_id).await.unwrap().unwrap();
    assert_eq!(loaded.task, task);
    assert_eq!(loaded.task.definition, Some(definition(1)));
    assert_eq!(loaded.task.input_refs, inputs);
    assert_eq!(
        loaded
            .inputs
            .iter()
            .map(|v| v.bytes.as_slice())
            .collect::<Vec<_>>(),
        vec![
            b"Other input".as_slice(),
            b"Original text".as_slice(),
            b"Revised text".as_slice()
        ]
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 2);
    assert_eq!(f.files(), 4);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_versions_coexist_without_latest_or_missing_pin_fallback() {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let first = f.create(token, vec![]).await;
    let both = SessionCatalogTaskWorkspace::new(
        f.task_workspace.clone(),
        f.artifact_workspace.clone(),
        &catalog(vec![definition(1), definition(2)]),
    )
    .unwrap();
    let second = f
        .auth
        .source
        .create_session_catalog_task(
            token,
            &both,
            id(),
            &definition(2).reference,
            "Definition two",
            vec![],
        )
        .await
        .unwrap();
    assert_eq!(second.definition, Some(definition(2)));
    assert_eq!(
        f.auth
            .source
            .get_session_catalog_task(token, &both, first.reference.task_id)
            .await
            .unwrap()
            .unwrap()
            .task,
        first
    );
    assert_eq!(
        f.auth
            .source
            .get_session_catalog_task(token, &both, second.reference.task_id)
            .await
            .unwrap()
            .unwrap()
            .task,
        second
    );
    assert_eq!(
        f.get(token, second.reference.task_id).await,
        Err(SessionTaskError::Task(TaskStoreError::UnknownDefinition))
    );
    let only_new = SessionCatalogTaskWorkspace::new(
        f.task_workspace.clone(),
        f.artifact_workspace.clone(),
        &catalog(vec![definition(2)]),
    )
    .unwrap();
    assert_eq!(
        f.auth
            .source
            .get_session_catalog_task(token, &only_new, first.reference.task_id)
            .await,
        Err(SessionTaskError::Task(TaskStoreError::UnknownDefinition))
    );
    assert_eq!(
        f.auth
            .source
            .create_session_catalog_task(
                token,
                &both,
                id(),
                &definition(3).reference,
                "No version fallback",
                vec![]
            )
            .await,
        Err(SessionTaskError::Task(TaskStoreError::UnknownDefinition))
    );
    assert_eq!(f.count_task("collaboration_tasks").await, 2);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 2);
    assert_eq!(f.files(), 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_and_manual_adapters_reject_each_others_definitions() {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let input = f.create_artifact(token, b"Optional content").await;
    let manual_inputs =
        SessionTaskInputsWorkspace::new(f.task_workspace.clone(), f.artifact_workspace.clone())
            .unwrap();
    for inputs in [vec![], vec![input.reference.clone()]] {
        let defined = f.create(token, inputs.clone()).await;
        assert_eq!(
            f.auth
                .source
                .get_session_manual_task(token, &f.task_workspace, defined.reference.task_id)
                .await,
            Err(SessionTaskError::UnsupportedTask)
        );
        assert_eq!(
            f.auth
                .source
                .transition_session_manual_task(
                    token,
                    &f.task_workspace,
                    defined.reference.task_id,
                    defined.revision,
                    TaskStatus::Ready
                )
                .await,
            Err(SessionTaskError::UnsupportedTask)
        );
        assert_eq!(
            f.auth
                .source
                .get_session_input_task(token, &manual_inputs, defined.reference.task_id)
                .await,
            Err(SessionTaskError::UnsupportedTask)
        );
        assert_eq!(
            f.auth
                .source
                .transition_session_input_task(
                    token,
                    &manual_inputs,
                    defined.reference.task_id,
                    defined.revision,
                    TaskStatus::Ready
                )
                .await,
            Err(SessionTaskError::UnsupportedTask)
        );
        let manual = f
            .tasks
            .create(
                id(),
                input.owner,
                TaskDraft::manual("Manual", inputs).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            f.get(token, manual.reference.task_id).await,
            Err(SessionTaskError::UnsupportedTask)
        );
        assert_eq!(
            f.transition(token, &manual, TaskStatus::Ready).await,
            Err(SessionTaskError::UnsupportedTask)
        );
    }
    assert_eq!(f.count_task("collaboration_task_outbox").await, 4);
    assert_eq!(f.files(), 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_revision_races_and_terminal_cancellation_preserve_definition(
) {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let first = f.create(token, vec![]).await;
    assert_eq!(
        f.request(token, first.reference.task_id, vec![]).await,
        Err(SessionTaskError::Task(TaskStoreError::Conflict))
    );
    let (a, b) = tokio::join!(
        f.transition(token, &first, TaskStatus::Ready),
        f.transition(token, &first, TaskStatus::Cancelled)
    );
    let second = match (a, b) {
        (Ok(winner), Err(SessionTaskError::Task(TaskStoreError::StaleRevision)))
        | (Err(SessionTaskError::Task(TaskStoreError::StaleRevision)), Ok(winner)) => winner,
        other => panic!("one revision must win: {other:?}"),
    };
    let terminal = if second.status == TaskStatus::Cancelled {
        second
    } else {
        f.transition(token, &second, TaskStatus::Cancelled)
            .await
            .unwrap()
    };
    assert_eq!(
        f.transition(token, &terminal, TaskStatus::Ready).await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidTransition))
    );
    let loaded = f
        .get(token, first.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.task, terminal);
    assert_eq!(loaded.task.definition, first.definition);
    assert!(loaded.inputs.is_empty());
    assert_eq!(
        f.count_task("collaboration_task_outbox").await,
        u64::from(terminal.revision) as i64
    );
    assert_eq!(f.files(), 0);
    f.cleanup().await;
}
