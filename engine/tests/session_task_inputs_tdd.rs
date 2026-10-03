#![cfg(unix)]
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore, ArtifactStoreError},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionArtifactWorkspace,
            SessionBindCommand, SessionCommandError, SessionPolicyCommand, SessionTaskError,
            SessionTaskInputs, SessionTaskInputsWorkspace, SessionTaskWorkspace,
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
#[path = "session_task_inputs/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "session_task_inputs/concurrency.rs"]
mod concurrency;
#[path = "session_task_inputs/faults.rs"]
mod faults;
#[path = "session_task_inputs/lifecycle.rs"]
mod lifecycle;

#[tokio::test]
async fn session_task_input_configuration_requires_distinct_namespaces_and_matching_scope() {
    let source = unavailable_source().await;
    let directory = private_directory();
    let artifacts = source
        .open_artifact_workspace("private_artifacts", scope(), &directory)
        .unwrap();
    let tasks = SessionTaskWorkspace::new("private_tasks", scope()).unwrap();
    assert!(SessionTaskInputsWorkspace::new(tasks, artifacts.clone()).is_ok());
    assert!(matches!(
        SessionTaskInputsWorkspace::new(
            SessionTaskWorkspace::new("private_artifacts", scope()).unwrap(),
            artifacts.clone()
        ),
        Err(SessionTaskError::InvalidNamespace)
    ));
    for other in [
        WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        WorkspaceRef {
            authority_id: id(),
            ..scope()
        },
    ] {
        assert!(SessionTaskInputsWorkspace::new(
            SessionTaskWorkspace::new("private_tasks", other).unwrap(),
            artifacts.clone()
        )
        .is_err());
    }
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop(artifacts);
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn unavailable_session_task_inputs_never_fall_back_or_publish_claimed_evidence() {
    let source = unavailable_source().await;
    let directory = private_directory();
    let workspace = offline_workspace(&source, &directory);
    let reference = ArtifactRef {
        workspace: scope(),
        artifact_id: id(),
        revision_id: id(),
        sha256: "a".repeat(64).parse().unwrap(),
    };
    assert_eq!(
        source
            .create_session_input_task(
                "invalid",
                &workspace,
                id(),
                "Draft",
                vec![reference.clone()]
            )
            .await,
        Err(SessionTaskError::Session(
            SessionCommandError::InvalidSession
        ))
    );
    let token = Uuid::new_v4().to_string();
    let unavailable = SessionTaskError::Session(SessionCommandError::Authentication(
        DurableAuthError::StorageUnavailable,
    ));
    assert_eq!(
        source
            .create_session_input_task(&token, &workspace, id(), "Draft", vec![reference])
            .await,
        Err(unavailable)
    );
    assert_eq!(
        source
            .get_session_input_task(&token, &workspace, id())
            .await,
        Err(unavailable)
    );
    assert_eq!(
        source
            .transition_session_input_task(
                &token,
                &workspace,
                id(),
                1.try_into().unwrap(),
                TaskStatus::Ready
            )
            .await,
        Err(unavailable)
    );
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop(workspace);
    fs::remove_dir(directory).unwrap();
}

#[test]
fn session_task_inputs_use_the_source_transaction_without_teaching_or_live_composition() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let adapter = fs::read_to_string(
        root.join("src/services/auth_service/durable_source/task_commands/inputs.rs"),
    )
    .unwrap();
    for forbidden in [
        "validate_session(",
        "command_actor(",
        "LegacyRole",
        "std::env",
        "AppState",
        "EbpfTeachingPack",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    for expected in [
        "begin_private_work",
        "read_in_transaction",
        "create_in_transaction",
        "transition_in_transaction",
        ".finish(",
    ] {
        assert!(adapter.contains(expected), "{expected}");
    }
    for file in ["src/application.rs", "src/state.rs"] {
        assert!(!fs::read_to_string(root.join(file))
            .unwrap()
            .contains("SessionTaskInputsWorkspace"));
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_pin_exact_versions_and_preserve_input_order() {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let first = f.create_artifact(token, b"First version").await;
    let other = f.create_artifact(token, b"Other document").await;
    let second = f
        .auth
        .source
        .revise_session_artifact(
            token,
            &f.artifact_workspace,
            &first.reference,
            id(),
            draft(b"Second version"),
        )
        .await
        .unwrap();
    let refs = vec![
        other.reference.clone(),
        first.reference.clone(),
        second.reference.clone(),
    ];
    let task = f.create(token, refs.clone()).await;
    assert_eq!(task.owner, f.auth.learner.principal.reference);
    assert_eq!(task.input_refs, refs);
    assert_eq!(task.definition, None);
    assert_eq!(task.status, TaskStatus::Draft);
    f.auth
        .source
        .revise_session_artifact(
            token,
            &f.artifact_workspace,
            &second.reference,
            id(),
            draft(b"Later version not selected"),
        )
        .await
        .unwrap();
    let observed: SessionTaskInputs = f.get(token, task.reference.task_id).await.unwrap().unwrap();
    assert_eq!(observed.task, task);
    assert_eq!(
        observed
            .inputs
            .iter()
            .map(|v| v.revision.reference.clone())
            .collect::<Vec<_>>(),
        refs
    );
    assert_eq!(
        observed
            .inputs
            .iter()
            .map(|v| v.bytes.as_slice())
            .collect::<Vec<_>>(),
        vec![
            b"Other document".as_slice(),
            b"First version".as_slice(),
            b"Second version".as_slice()
        ]
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.files(), 4);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_are_private_without_teaching_or_deployment_grants() {
    let f = Fixture::ready().await;
    let policy = f
        .auth
        .current(f.auth.learner.binding.principal_id)
        .await
        .policy;
    assert!(policy.membership.role_refs.is_empty());
    assert!(!policy.deployment_granted);
    let input = f
        .create_artifact(&f.auth.learner_token, b"Private document")
        .await;
    let task = f
        .create(&f.auth.learner_token, vec![input.reference.clone()])
        .await;
    fs::remove_file(f.blob(&input.reference)).unwrap();
    assert_eq!(
        f.get(&f.auth.manager_token, task.reference.task_id)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        f.transition(&f.auth.manager_token, &task, TaskStatus::Ready)
            .await,
        Err(SessionTaskError::Task(TaskStoreError::NotFound))
    );
    assert_eq!(
        f.get(&f.auth.learner_token, task.reference.task_id).await,
        Err(SessionTaskError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    assert_eq!(f.files(), 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_reject_missing_foreign_or_mismatched_evidence_atomically() {
    let f = Fixture::ready().await;
    let own = f
        .create_artifact(&f.auth.learner_token, b"Same bytes")
        .await;
    let foreign = f
        .create_artifact(&f.auth.manager_token, b"Same bytes")
        .await;
    assert_eq!(own.reference.sha256, foreign.reference.sha256);
    let missing = ArtifactRef {
        artifact_id: id(),
        ..own.reference.clone()
    };
    let missing_revision = ArtifactRef {
        revision_id: id(),
        ..own.reference.clone()
    };
    let cross_scope = ArtifactRef {
        workspace: WorkspaceRef {
            workspace_id: id(),
            ..scope()
        },
        ..own.reference.clone()
    };
    let wrong_digest = ArtifactRef {
        sha256: "f".repeat(64).parse().unwrap(),
        ..own.reference.clone()
    };
    for invalid in [
        foreign.reference,
        missing.clone(),
        missing_revision,
        cross_scope,
        wrong_digest,
    ] {
        assert!(f
            .request(&f.auth.learner_token, id(), vec![invalid])
            .await
            .is_err());
        assert_eq!(f.count_task("collaboration_tasks").await, 0);
        assert_eq!(f.count_task("collaboration_task_outbox").await, 0);
    }
    // A preexisting trusted-store snapshot is also not a reason to return partial evidence.
    let unresolved = f
        .tasks
        .create(
            id(),
            own.owner,
            TaskDraft::manual("Unresolved", vec![own.reference, missing]).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        f.get(&f.auth.learner_token, unresolved.reference.task_id)
            .await,
        Err(SessionTaskError::Artifact(ArtifactStoreError::NotFound))
    );
    assert_eq!(
        f.transition(&f.auth.learner_token, &unresolved, TaskStatus::Ready)
            .await,
        Err(SessionTaskError::Artifact(ArtifactStoreError::NotFound))
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 2);
    assert_eq!(f.files(), 2);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_keep_manual_and_domain_adapters_strictly_separate() {
    use cyanrex_engine::{
        domain_packs::ebpf_teaching::EbpfTeachingPack, services::task_catalog::TaskCatalog,
    };
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let input = f.create_artifact(token, b"Input").await;
    let task = f.create(token, vec![input.reference.clone()]).await;
    assert_eq!(
        f.auth
            .source
            .get_session_manual_task(token, &f.task_workspace, task.reference.task_id)
            .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    assert_eq!(
        f.auth
            .source
            .transition_session_manual_task(
                token,
                &f.task_workspace,
                task.reference.task_id,
                task.revision,
                TaskStatus::Ready
            )
            .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    assert_eq!(
        f.request(token, id(), vec![]).await,
        Err(SessionTaskError::UnsupportedTask)
    );
    let catalog = TaskCatalog::new(EbpfTeachingPack).unwrap();
    let definition = EbpfTeachingPack::definition_ref("01-first-program").unwrap();
    for draft in [
        TaskDraft::manual("No inputs", vec![]).unwrap(),
        TaskDraft::from_catalog(&catalog, &definition, "Domain task", vec![input.reference])
            .unwrap(),
    ] {
        let stored = f.tasks.create(id(), task.owner, draft).await.unwrap();
        assert_eq!(
            f.get(token, stored.reference.task_id).await,
            Err(SessionTaskError::UnsupportedTask)
        );
        assert_eq!(
            f.transition(token, &stored, TaskStatus::Ready).await,
            Err(SessionTaskError::UnsupportedTask)
        );
    }
    assert_eq!(f.count_task("collaboration_tasks").await, 3);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 3);
    assert_eq!(f.files(), 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_bound_evidence_count_and_reject_duplicate_coordinates() {
    let f = Fixture::ready().await;
    let mut inputs = Vec::new();
    for value in 0..32u8 {
        inputs.push(
            f.create_artifact(&f.auth.learner_token, &[value])
                .await
                .reference,
        );
    }
    let task = f.create(&f.auth.learner_token, inputs.clone()).await;
    let loaded = f
        .get(&f.auth.learner_token, task.reference.task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.inputs.len(), 32);
    assert_eq!(
        loaded.inputs.iter().map(|v| v.bytes[0]).collect::<Vec<_>>(),
        (0..32u8).collect::<Vec<_>>()
    );
    let mut excessive = inputs.clone();
    excessive.push(ArtifactRef {
        artifact_id: id(),
        ..inputs[0].clone()
    });
    let different_digest = ArtifactRef {
        sha256: "f".repeat(64).parse().unwrap(),
        ..inputs[0].clone()
    };
    for invalid in [
        excessive,
        vec![inputs[0].clone(), inputs[0].clone()],
        vec![inputs[0].clone(), different_digest],
    ] {
        assert_eq!(
            f.request(&f.auth.learner_token, id(), invalid).await,
            Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
        );
    }
    assert_eq!(f.count_task("collaboration_tasks").await, 1);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.files(), 32);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_revision_races_and_terminal_states_have_one_winner() {
    let f = Fixture::ready().await;
    let token = &f.auth.learner_token;
    let input = f.create_artifact(token, b"Pinned input").await;
    let task = f.create(token, vec![input.reference]).await;
    assert_eq!(
        f.request(token, task.reference.task_id, task.input_refs.clone())
            .await,
        Err(SessionTaskError::Task(TaskStoreError::Conflict))
    );
    let (a, b) = tokio::join!(
        f.transition(token, &task, TaskStatus::Ready),
        f.transition(token, &task, TaskStatus::Cancelled)
    );
    let second = match (a, b) {
        (Ok(winner), Err(SessionTaskError::Task(TaskStoreError::StaleRevision)))
        | (Err(SessionTaskError::Task(TaskStoreError::StaleRevision)), Ok(winner)) => winner,
        other => panic!("one transition must win: {other:?}"),
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
    let loaded = f.get(token, task.reference.task_id).await.unwrap().unwrap();
    assert_eq!(loaded.task, terminal);
    assert_eq!(loaded.task.input_refs, task.input_refs);
    assert_eq!(loaded.inputs[0].bytes, b"Pinned input");
    assert_eq!(
        f.count_task("collaboration_task_outbox").await,
        u64::from(terminal.revision) as i64
    );
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    f.cleanup().await;
}
