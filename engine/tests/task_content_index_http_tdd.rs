#![cfg(unix)]
//! Standalone private metadata transport; no live Engine composition or saved-content receipt.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    platform_http::{
        build_catalog_task_content_router, build_task_content_index_router,
        build_task_content_router, build_text_artifact_router, CatalogTaskContentHttpState,
        TaskContentHttpConfigError, TaskContentHttpState, TaskContentIndexHttpState,
        TextArtifactHttpState,
    },
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthSource, SessionArtifactWorkspace, SessionBindCommand,
            SessionCatalogTaskContentWorkspace, SessionPolicyCommand, SessionTaskContent,
            SessionTaskContentIndexWorkspace, SessionTaskContentWorkspace, SessionTaskError,
            SessionTaskInputs, SessionTaskInputsWorkspace, SessionTaskWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        task_catalog::TaskDefinitionCatalog,
        task_store::{TaskContentIndexEntry, TaskContentSnapshot, TaskContentStore, TaskStore},
    },
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use std::{fs, path::PathBuf, time::Duration};
use tower::ServiceExt;
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
use content_fixture::Fixture;
#[path = "task_content_index_http/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "session_task_content_index/fixture.rs"]
mod index_fixture;
use index_fixture::manifest;
#[allow(dead_code)]
#[path = "session_task_content_index/sql_fixture.rs"]
mod index_sql_fixture;
use fixture::*;
#[path = "task_content_index_http/admission.rs"]
mod admission;
#[path = "task_content_index_http/artifact_independence.rs"]
mod artifact_independence;
#[path = "task_content_index_http/behavior.rs"]
mod behavior;
#[path = "task_content_index_http/bodies.rs"]
mod bodies;
#[path = "task_content_index_http/capacity_sql.rs"]
mod capacity_sql;
#[path = "task_content_index_http/defaults.rs"]
mod defaults;
#[path = "task_content_index_http/faults.rs"]
mod faults;
#[path = "task_content_index_http/lifecycle.rs"]
mod lifecycle;
#[path = "task_content_index_http/privacy.rs"]
mod privacy;
#[path = "task_content_index_http/waits.rs"]
mod waits;
