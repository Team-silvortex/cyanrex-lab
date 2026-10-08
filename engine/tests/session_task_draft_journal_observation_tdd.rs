#![cfg(unix)]
//! Current authorized resource observation of one recorded ordinal, never acknowledgement/retry.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::{ArtifactDraft, ArtifactStore, ArtifactStoreError},
        auth_service::durable_source::*,
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            StoredLegacyAccessPolicy, StoredLegacyIdentity,
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
use publication_fixture::{backend, blocked_by, plan, prepare, wait_expired};
#[allow(dead_code)]
#[path = "session_task_draft_dispatch/fixture.rs"]
mod dispatch_fixture;
#[allow(dead_code)]
#[path = "session_task_draft_intent/fixture.rs"]
mod intent_fixture;
#[allow(dead_code)]
#[path = "session_task_draft_journal_inspection/fixture.rs"]
mod journal_fixture;
use journal_fixture::snapshot;
#[path = "session_task_draft_journal_observation/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "session_task_draft_journal_observation/admission.rs"]
mod admission;
#[path = "session_task_draft_journal_observation/behavior.rs"]
mod behavior;
#[path = "session_task_draft_journal_observation/concurrency.rs"]
mod concurrency;
#[path = "session_task_draft_journal_observation/faults.rs"]
mod faults;
#[path = "session_task_draft_journal_observation/lifecycle.rs"]
mod lifecycle;
