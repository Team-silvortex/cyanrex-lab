#![cfg(unix)]
//! Unmounted exact single-chunk binary adapter; in-process response evidence, never LAN delivery.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    platform_http::{
        build_file_chunk_read_router, FileChunkReadHttpState, TaskContentHttpConfigError,
    },
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
        file_publisher_grant_store::{FilePublisherGrantStore, StoredFilePublisherGrant},
        file_recipient_grant_store::FileRecipientGrantStore,
        file_transfer::staging::ChunkStaging,
        file_upload_publication::FileUploadPublicationStore,
    },
};
use http_body_util::BodyExt;
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
#[path = "file_chunk_read_http/authorization.rs"]
mod authorization;
#[path = "file_chunk_read_http/baseline.rs"]
mod baseline;
#[path = "file_chunk_read_http/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[allow(dead_code)]
#[path = "session_file_chunk_read/fixture.rs"]
mod chunk_fixture;
#[path = "file_chunk_read_http/defaults_admission.rs"]
mod defaults_admission;
#[path = "file_chunk_read_http/defaults_bodies.rs"]
mod defaults_bodies;
#[path = "file_chunk_read_http/defaults_config.rs"]
mod defaults_config;
#[path = "file_chunk_read_http/defaults_endpoint.rs"]
mod defaults_endpoint;
#[path = "file_chunk_read_http/defaults_headers.rs"]
mod defaults_headers;
#[path = "file_chunk_read_http/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "session_file_publisher_grant/fixture.rs"]
mod grant_fixture;
#[path = "file_chunk_read_http/offline.rs"]
mod offline;
#[allow(dead_code)]
#[path = "session_file_publication/fixture.rs"]
mod publication_fixture;
#[path = "file_chunk_read_http/response_lifetime.rs"]
mod response_lifetime;
#[allow(dead_code)]
#[path = "session_file_sharing/fixture.rs"]
mod sharing_fixture;
#[allow(dead_code)]
#[path = "session_file_chunk_read/pool_gate.rs"]
mod source_gate;
#[path = "file_chunk_read_http/storage.rs"]
mod storage;
#[path = "file_chunk_read_http/support.rs"]
mod support;
#[path = "file_chunk_read_http/waits.rs"]
mod waits;
