#![cfg(unix)]
//! Explicit current-Session reads of untrusted checkpoint metadata, not attempt restoration.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore, ArtifactStoreError},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionArtifactError,
            SessionArtifactWorkspace, SessionBindCommand, SessionCommandError,
            SessionDraftCheckpointInspectionError, SessionDraftCheckpointObservation,
            SessionDraftCheckpointObservedOutcome, SessionDraftObservedOutcome,
            SessionDraftPublicationCheckpoint, SessionDraftPublicationError,
            SessionDraftPublicationObservation, SessionDraftPublicationState,
            SessionDraftPublicationStep, SessionPolicyCommand, SessionTaskContent,
            SessionTaskContentWorkspace, SessionTaskDraftPublication, SessionTaskError,
            SessionTaskInputs, SessionTaskInputsWorkspace, SessionTaskWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            StoredLegacyAccessPolicy, StoredLegacyIdentity,
        },
        task_draft_import::TaskDraftPublicationPlan,
        task_store::{TaskContentSnapshot, TaskContentStore, TaskStore, TaskStoreError},
    },
};
use serde_json::{json, Value};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{fs, path::PathBuf, time::Duration};
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
#[path = "session_task_content/fixture.rs"]
mod content_fixture;
#[allow(dead_code)]
#[path = "session_task_inputs/fixture.rs"]
mod input_fixture;
#[allow(dead_code)]
#[path = "session_task_draft_publication/fixture.rs"]
mod publication_fixture;
use publication_fixture::*;
#[allow(dead_code)]
#[path = "session_task_draft_observation/fixture.rs"]
mod observation_fixture;
use observation_fixture::{
    create_target, damage, publish_target, snapshot, unknown_artifact, unknown_task, ITEMS,
};
#[path = "session_task_draft_inspection/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "session_task_draft_inspection/behavior.rs"]
mod behavior;
#[path = "session_task_draft_inspection/concurrency.rs"]
mod concurrency;
#[path = "session_task_draft_inspection/faults.rs"]
mod faults;
#[path = "session_task_draft_inspection/lifecycle.rs"]
mod lifecycle;
