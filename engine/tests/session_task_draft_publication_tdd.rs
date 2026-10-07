#![cfg(unix)]
//! Explicit caller-owned publication orchestration, not HTTP or an atomic multi-write workflow.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore, ArtifactStoreError},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionArtifactError,
            SessionArtifactWorkspace, SessionBindCommand, SessionCommandError,
            SessionDraftPublicationError, SessionDraftPublicationProgress,
            SessionDraftPublicationState, SessionDraftPublicationStep, SessionPolicyCommand,
            SessionTaskContent, SessionTaskContentWorkspace, SessionTaskDraftPublication,
            SessionTaskError, SessionTaskInputs, SessionTaskInputsWorkspace, SessionTaskWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            StoredLegacyAccessPolicy, StoredLegacyIdentity,
        },
        task_draft_import::TaskDraftPublicationPlan,
        task_store::{TaskContentSnapshot, TaskContentStore, TaskStore},
    },
};
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
#[path = "session_task_draft_publication/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "session_task_inputs/fixture.rs"]
mod input_fixture;
use fixture::*;
#[path = "session_task_draft_publication/behavior.rs"]
mod behavior;
#[path = "session_task_draft_publication/cancellation.rs"]
mod cancellation;
#[path = "session_task_draft_publication/faults.rs"]
mod faults;
#[path = "session_task_draft_publication/lifecycle.rs"]
mod lifecycle;
