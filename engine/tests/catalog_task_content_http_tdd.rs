#![cfg(unix)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    platform_http::{
        build_catalog_task_content_router, build_task_content_router, CatalogTaskContentHttpState,
        TaskContentHttpState,
    },
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthSource, SessionArtifactWorkspace, SessionBindCommand,
            SessionCatalogTaskContentWorkspace, SessionPolicyCommand, SessionTaskContent,
            SessionTaskContentWorkspace, SessionTaskError, SessionTaskInputs,
            SessionTaskInputsWorkspace, SessionTaskWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, LegacyIdentityAction,
            LegacyIdentityCommand, StoredLegacyAccessPolicy, StoredLegacyIdentity,
        },
        task_catalog::TaskDefinitionCatalog,
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
#[path = "catalog_task_content_http/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "platform_http/fixture.rs"]
mod manual_http_fixture;
use fixture::*;
#[path = "catalog_task_content_http/behavior.rs"]
mod behavior;
#[path = "catalog_task_content_http/bodies.rs"]
mod bodies;
#[path = "catalog_task_content_http/capacity_sql.rs"]
mod capacity_sql;
#[path = "catalog_task_content_http/defaults.rs"]
mod defaults;
#[path = "catalog_task_content_http/faults.rs"]
mod faults;
#[path = "catalog_task_content_http/lifecycle.rs"]
mod lifecycle;
#[path = "catalog_task_content_http/metadata.rs"]
mod metadata;
#[path = "catalog_task_content_http/privacy.rs"]
mod privacy;
#[path = "catalog_task_content_http/references.rs"]
mod references;
#[path = "catalog_task_content_http/shared.rs"]
mod shared;
#[path = "catalog_task_content_http/validation.rs"]
mod validation;
#[path = "catalog_task_content_http/waits.rs"]
mod waits;
