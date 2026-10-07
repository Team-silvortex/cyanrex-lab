#![cfg(unix)]
//! Metadata-only checkpoint parsing never restores a publication capability or authenticates claims.
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAuthSource, SessionDraftCheckpointState, SessionDraftPublicationCheckpoint,
            SessionDraftPublicationState, SessionDraftPublicationStep, SessionTaskContentWorkspace,
            SessionTaskDraftPublication, MAX_SESSION_DRAFT_CHECKPOINT_BYTES,
        },
        task_draft_import::TaskDraftPublicationPlan,
    },
};
use serde_json::{json, Value};
use sqlx_postgres::PgPoolOptions;
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};
use uuid::Uuid;

#[path = "session_task_draft_checkpoint/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "session_task_draft_checkpoint/export.rs"]
mod export;
#[path = "session_task_draft_checkpoint/parsing.rs"]
mod parsing;
#[path = "session_task_draft_checkpoint/progress.rs"]
mod progress;
