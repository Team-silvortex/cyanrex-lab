#![cfg(unix)]
//! Explicit in-process chunk reception into a distinct private root, not network delivery.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    platform_http::{build_file_chunk_read_router, FileChunkReadHttpState},
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthSource, SessionBindCommand, SessionFileChunkReadRequest,
            SessionFileChunkReadWorkspace, SessionFileDownloadRequest,
            SessionFilePublisherGrantWorkspace, SessionFilePublishingWorkspace,
            SessionFileRecipientGrantIssue, SessionFileRecipientGrantWorkspace,
            SessionFileSharingWorkspace, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        file_chunk_receiver::{FileChunkReceiveError, FileChunkReceiver},
        file_publisher_grant_store::{FilePublisherGrantStore, StoredFilePublisherGrant},
        file_recipient_grant_store::FileRecipientGrantStore,
        file_transfer::{staging::ChunkStaging, verify_chunk},
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
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
#[path = "file_chunk_receiver/admission.rs"]
mod admission;
#[path = "file_chunk_receiver/baseline.rs"]
mod baseline;
#[path = "file_chunk_receiver/bodies.rs"]
mod bodies;
#[path = "file_chunk_receiver/bridge.rs"]
mod bridge;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[allow(dead_code)]
#[path = "session_file_chunk_read/fixture.rs"]
mod chunk_fixture;
#[path = "file_chunk_receiver/deadlines.rs"]
mod deadlines;
#[path = "file_chunk_receiver/defaults.rs"]
mod defaults;
#[path = "file_chunk_receiver/filesystem.rs"]
mod filesystem;
#[path = "file_chunk_receiver/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "session_file_publisher_grant/fixture.rs"]
mod grant_fixture;
#[path = "file_chunk_receiver/headers.rs"]
mod headers;
#[allow(dead_code)]
#[path = "file_chunk_read_http/fixture.rs"]
mod http_fixture;
#[path = "file_chunk_receiver/local.rs"]
mod local;
#[allow(dead_code)]
#[path = "session_file_publication/fixture.rs"]
mod publication_fixture;
#[path = "file_chunk_receiver/script.rs"]
mod script;
#[allow(dead_code)]
#[path = "session_file_sharing/fixture.rs"]
mod sharing_fixture;
