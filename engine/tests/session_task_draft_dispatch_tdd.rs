#![cfg(unix)]
//! Journal-gated two-transaction dispatch; no restore, retry or acknowledgement by inference.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore},
        auth_service::durable_source::*,
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        draft_intent_journal::{DraftIntentJournal, DraftIntentJournalError},
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
#[allow(dead_code)]
#[path = "session_task_inputs/fixture.rs"]
mod input_fixture;
#[allow(dead_code)]
#[path = "session_task_draft_publication/fixture.rs"]
mod publication_fixture;
use publication_fixture::{backend, blocked_by, counts, plan, prepare, wait_expired};
#[allow(dead_code)]
#[path = "session_task_draft_intent/fixture.rs"]
mod intent_fixture;

#[path = "session_task_draft_dispatch/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "session_task_draft_dispatch/admission.rs"]
mod admission;
#[path = "session_task_draft_dispatch/behavior.rs"]
mod behavior;
#[path = "session_task_draft_dispatch/concurrency.rs"]
mod concurrency;
#[path = "session_task_draft_dispatch/faults.rs"]
mod faults;
#[path = "session_task_draft_dispatch/lifecycle.rs"]
mod lifecycle;
