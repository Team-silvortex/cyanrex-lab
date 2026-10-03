#![cfg(unix)]
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore, ArtifactStoreError},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionArtifactWorkspace,
            SessionBindCommand, SessionCatalogTaskWorkspace, SessionCommandError,
            SessionPolicyCommand, SessionReviewWorkspace, SessionTaskError, SessionTaskInputs,
            SessionTaskInputsWorkspace, SessionTaskWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, LegacyIdentityAction,
            LegacyIdentityCommand, StoredLegacyAccessPolicy, StoredLegacyIdentity,
        },
        review_store::{HumanReviewEdit, ReviewStore},
        task_catalog::{TaskCatalog, TaskCatalogError, TaskProvider},
        task_store::{TaskStore, TaskStoreError},
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
#[allow(dead_code)]
#[path = "session_task_inputs/fixture.rs"]
mod input_fixture;
use input_fixture::*;
#[allow(dead_code)]
#[path = "session_catalog_task/fixture.rs"]
mod catalog_fixture;
#[path = "session_task_revision/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "session_task_revision/behavior.rs"]
mod behavior;
#[path = "session_task_revision/concurrency.rs"]
mod concurrency;
#[path = "session_task_revision/faults.rs"]
mod faults;
#[path = "session_task_revision/lifecycle.rs"]
mod lifecycle;

fn synthetic_ref() -> ArtifactRef {
    ArtifactRef {
        workspace: scope(),
        artifact_id: id(),
        revision_id: id(),
        sha256: "a".repeat(64).parse().unwrap(),
    }
}

#[tokio::test]
async fn session_task_replacement_rejects_invalid_shapes_without_a_database() {
    let source = unavailable_source().await;
    let directory = private_directory();
    let manual = offline_workspace(&source, &directory);
    let catalog = catalog_fixture::offline_workspace(&source, &directory);
    let token = Uuid::new_v4().to_string();
    let revision = 1.try_into().unwrap();
    assert_eq!(
        source
            .replace_session_input_task_inputs(&token, &manual, id(), revision, vec![])
            .await,
        Err(SessionTaskError::UnsupportedTask)
    );
    let reference = synthetic_ref();
    for refs in [
        vec![reference.clone(); 2],
        (0..33).map(|_| synthetic_ref()).collect(),
    ] {
        assert_eq!(
            source
                .replace_session_input_task_inputs(&token, &manual, id(), revision, refs.clone())
                .await,
            Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
        );
        assert_eq!(
            source
                .replace_session_catalog_task_inputs(&token, &catalog, id(), revision, refs)
                .await,
            Err(SessionTaskError::Task(TaskStoreError::InvalidInput))
        );
    }
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop((manual, catalog));
    fs::remove_dir(directory).unwrap();
}

#[tokio::test]
async fn session_task_replacement_fails_closed_for_invalid_or_unavailable_sessions() {
    let source = unavailable_source().await;
    let directory = private_directory();
    let manual = offline_workspace(&source, &directory);
    let catalog = catalog_fixture::offline_workspace(&source, &directory);
    for (token, error) in [
        ("invalid".to_string(), SessionCommandError::InvalidSession),
        (
            Uuid::new_v4().to_string(),
            SessionCommandError::Authentication(DurableAuthError::StorageUnavailable),
        ),
    ] {
        assert_eq!(
            source
                .replace_session_input_task_inputs(
                    &token,
                    &manual,
                    id(),
                    1.try_into().unwrap(),
                    vec![synthetic_ref()]
                )
                .await,
            Err(SessionTaskError::Session(error))
        );
        assert_eq!(
            source
                .replace_session_catalog_task_inputs(
                    &token,
                    &catalog,
                    id(),
                    1.try_into().unwrap(),
                    vec![]
                )
                .await,
            Err(SessionTaskError::Session(error))
        );
    }
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
    drop((manual, catalog));
    fs::remove_dir(directory).unwrap();
}
