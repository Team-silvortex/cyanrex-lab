#![cfg(unix)]
//! Explicit standalone transport preparation; not mounted by the live Engine application.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    platform_http::{build_task_content_router, TaskContentHttpState},
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
#[path = "platform_http/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "platform_http/behavior.rs"]
mod behavior;
#[path = "platform_http/bodies.rs"]
mod bodies;
#[path = "platform_http/faults.rs"]
mod faults;
#[path = "platform_http/transport.rs"]
mod transport;
