#![cfg(unix)]
//! Explicit schema-3 Session preparation, not a public API or live authentication cutover.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore, ArtifactStoreError},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionArtifactWorkspace,
            SessionBindCommand, SessionCommandError, SessionPolicyCommand, SessionTaskContent,
            SessionTaskContentWorkspace, SessionTaskError, SessionTaskInputs,
            SessionTaskInputsWorkspace, SessionTaskWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, LegacyIdentityAction,
            LegacyIdentityCommand, StoredLegacyAccessPolicy, StoredLegacyIdentity,
        },
        task_store::{TaskContentSnapshot, TaskContentStore, TaskStore, TaskStoreError},
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
#[path = "session_task_content/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "session_task_inputs/fixture.rs"]
mod input_fixture;
use fixture::*;
#[path = "session_task_content/behavior.rs"]
mod behavior;
#[path = "session_task_content/concurrency.rs"]
mod concurrency;
#[path = "session_task_content/faults.rs"]
mod faults;
#[path = "session_task_content/lifecycle.rs"]
mod lifecycle;
#[path = "session_task_content/namespaces.rs"]
mod namespaces;

#[tokio::test]
async fn session_content_configuration_requires_separate_valid_namespaces_and_matching_scope() {
    let source = input_fixture::unavailable_source().await;
    let directory = input_fixture::private_directory();
    let artifacts = source
        .open_artifact_workspace("private_artifacts", scope(), &directory)
        .unwrap();
    assert!(
        SessionTaskContentWorkspace::new("private_content", scope(), artifacts.clone()).is_ok()
    );
    for name in [
        "",
        "public",
        "pg_catalog",
        "a.b",
        "has space",
        "private_artifacts",
    ] {
        assert!(
            matches!(
                SessionTaskContentWorkspace::new(name, scope(), artifacts.clone()),
                Err(SessionTaskError::InvalidNamespace)
            ),
            "{name}"
        );
    }
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
        assert!(
            SessionTaskContentWorkspace::new("private_content", other, artifacts.clone()).is_err()
        );
    }
    drop(artifacts);
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn unavailable_session_content_never_falls_back_or_returns_unverified_bytes() {
    use std::os::unix::fs::DirBuilderExt;
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://synthetic@127.0.0.1/unused")
        .unwrap();
    pool.close().await;
    let source = DurableAuthSource::new(pool, scope().authority_id);
    let directory =
        std::env::temp_dir().join(format!("cyanrex-session-content-{}", uuid::Uuid::new_v4()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let artifact = source
        .open_artifact_workspace("private_artifacts", scope(), &directory)
        .unwrap();
    let workspace = SessionTaskContentWorkspace::new("private_tasks", scope(), artifact).unwrap();
    let task_id = TaskId::try_from(uuid::Uuid::new_v4()).unwrap();
    for (token, expected) in [
        (
            "invalid".to_string(),
            SessionTaskError::Session(SessionCommandError::InvalidSession),
        ),
        (
            uuid::Uuid::new_v4().to_string(),
            SessionTaskError::Session(SessionCommandError::Authentication(
                DurableAuthError::StorageUnavailable,
            )),
        ),
    ] {
        let manifest = TaskContentManifest::new("Empty task", vec![]).unwrap();
        assert_eq!(
            source
                .create_session_content_task(&token, &workspace, task_id, manifest.clone())
                .await,
            Err(expected)
        );
        let read: Result<Option<SessionTaskContent>, SessionTaskError> = source
            .get_session_content_task(&token, &workspace, task_id)
            .await;
        assert_eq!(read, Err(expected));
        assert_eq!(
            source
                .replace_session_content_task(
                    &token,
                    &workspace,
                    task_id,
                    1.try_into().unwrap(),
                    manifest
                )
                .await,
            Err(expected)
        );
        assert_eq!(
            source
                .transition_session_content_task(
                    &token,
                    &workspace,
                    task_id,
                    1.try_into().unwrap(),
                    TaskStatus::Ready
                )
                .await,
            Err(expected)
        );
    }
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    drop(workspace);
    std::fs::remove_dir(directory).unwrap();
}
