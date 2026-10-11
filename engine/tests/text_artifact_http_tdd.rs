#![cfg(unix)]
//! Explicit text-only Artifact transport preparation; never a live application mount.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    platform_http::{
        build_task_content_router, build_text_artifact_router, TaskContentHttpConfigError,
        TaskContentHttpState, TextArtifactHttpState,
    },
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthSource, SessionArtifactWorkspace, SessionBindCommand,
            SessionPolicyCommand, SessionTaskContent, SessionTaskContentWorkspace,
            SessionTaskError, SessionTaskInputs, SessionTaskInputsWorkspace, SessionTaskWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        task_store::{TaskContentSnapshot, TaskContentStore, TaskStore},
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
use content_fixture::{manifest, Fixture};
#[path = "text_artifact_http/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_http/fixture.rs"]
mod http_fixture;
use fixture::*;
#[path = "text_artifact_http/admission.rs"]
mod admission;
#[path = "text_artifact_http/behavior.rs"]
mod behavior;
#[path = "text_artifact_http/bodies.rs"]
mod bodies;
#[path = "text_artifact_http/concurrency.rs"]
mod concurrency;
#[path = "text_artifact_http/faults.rs"]
mod faults;
#[path = "text_artifact_http/transport.rs"]
mod transport;
