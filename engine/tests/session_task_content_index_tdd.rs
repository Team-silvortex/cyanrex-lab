#![cfg(unix)]
//! Owner-private metadata index, not public HTTP or evidence that content can be reopened.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionArtifactWorkspace,
            SessionBindCommand, SessionCatalogTaskContentWorkspace, SessionCommandError,
            SessionPolicyCommand, SessionTaskContent, SessionTaskContentIndexWorkspace,
            SessionTaskContentWorkspace, SessionTaskError, SessionTaskInputs,
            SessionTaskInputsWorkspace, SessionTaskWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        task_catalog::TaskDefinitionCatalog,
        task_store::{
            TaskContentIndexEntry, TaskContentSnapshot, TaskContentStore, TaskStore, TaskStoreError,
        },
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
#[path = "session_task_content_index/artifact_independence.rs"]
mod artifact_independence;
#[path = "session_task_content_index/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "session_task_content/fixture.rs"]
mod content_fixture;
#[path = "session_task_content_index/defaults.rs"]
mod defaults;
#[allow(dead_code)]
#[path = "session_task_content_index/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "session_task_inputs/fixture.rs"]
mod input_fixture;
#[path = "session_task_content_index/lifecycle.rs"]
mod lifecycle;
#[path = "session_task_content_index/sql_fixture.rs"]
mod sql_fixture;
#[path = "session_task_content_index/validation.rs"]
mod validation;
#[path = "session_task_content_index/waits.rs"]
mod waits;
