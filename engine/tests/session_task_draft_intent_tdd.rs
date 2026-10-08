#![cfg(unix)]
//! Session-owned create-only intent registration, not publication, recovery or a progress journal.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore},
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthSource, SessionArtifactWorkspace, SessionBindCommand,
            SessionCommandError, SessionDraftCheckpointState, SessionDraftIntentError,
            SessionDraftIntentWorkspace, SessionDraftPublicationIntent,
            SessionDraftPublicationState, SessionDraftPublicationStep, SessionPolicyCommand,
            SessionTaskContent, SessionTaskContentWorkspace, SessionTaskDraftPublication,
            SessionTaskError, SessionTaskInputs, SessionTaskInputsWorkspace, SessionTaskWorkspace,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            StoredLegacyAccessPolicy, StoredLegacyIdentity,
        },
        draft_intent_journal::{DraftIntentJournal, DraftIntentJournalError},
        task_draft_import::TaskDraftPublicationPlan,
        task_store::{TaskContentSnapshot, TaskContentStore, TaskStore},
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
use publication_fixture::{backend, blocked_by, counts, plan, prepare, wait_expired};

#[path = "session_task_draft_intent/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "session_task_draft_intent/admission.rs"]
mod admission;
#[path = "session_task_draft_intent/behavior.rs"]
mod behavior;
#[path = "session_task_draft_intent/concurrency.rs"]
mod concurrency;
#[path = "session_task_draft_intent/faults.rs"]
mod faults;
#[path = "session_task_draft_intent/lifecycle.rs"]
mod lifecycle;
