#![cfg(unix)]
//! Explicit unmounted current-recipient metadata GET, not byte health or a reusable permission.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    platform_http::{
        build_file_download_manifest_router, FileDownloadManifestHttpState,
        TaskContentHttpConfigError,
    },
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthSource, SessionBindCommand, SessionCommandError,
            SessionFileDownloadError, SessionFileDownloadRequest, SessionFileDownloadWorkspace,
            SessionFilePublisherGrantWorkspace, SessionFilePublishingWorkspace,
            SessionFileRecipientGrantIssue, SessionFileRecipientGrantWorkspace,
            SessionFileSharingWorkspace, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            LegacyIdentityAction, LegacyIdentityCommand, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        file_publisher_grant_store::{FilePublisherGrantStore, StoredFilePublisherGrant},
        file_recipient_grant_store::{
            FileRecipientGrantStore, FileRecipientGrantStoreError, StoredFileRecipientGrant,
        },
        file_upload_publication::{FileUploadPublicationStore, FileUploadPublicationStoreError},
    },
};
use http_body_util::BodyExt;
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

#[path = "file_download_manifest_http/admission.rs"]
mod admission;
#[path = "file_download_manifest_http/baseline.rs"]
mod baseline;
#[path = "file_download_manifest_http/bodies.rs"]
mod bodies;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "file_download_manifest_http/credentials.rs"]
mod credentials;
#[path = "file_download_manifest_http/defaults.rs"]
mod defaults;
#[allow(dead_code)]
#[path = "session_file_download/fixture.rs"]
mod download_fixture;
#[path = "file_download_manifest_http/selectors.rs"]
mod selectors;
use download_fixture::scope;
#[path = "file_download_manifest_http/authorization.rs"]
mod authorization;
#[path = "file_download_manifest_http/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "session_file_download/support.rs"]
mod download_support;
#[path = "file_download_manifest_http/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "session_file_publisher_grant/fixture.rs"]
mod grant_fixture;
#[allow(dead_code)]
#[path = "durable_collaboration/fixture.rs"]
mod identity_fixture;
#[path = "file_download_manifest_http/lifetime.rs"]
mod lifetime;
#[path = "file_download_manifest_http/offline.rs"]
mod offline;
#[allow(dead_code)]
#[path = "session_file_publication/fixture.rs"]
mod publication_fixture;
#[allow(dead_code)]
#[path = "session_file_sharing/fixture.rs"]
mod sharing_fixture;
#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
#[path = "file_download_manifest_http/storage.rs"]
mod storage;
#[path = "file_download_manifest_http/support.rs"]
mod support;
#[path = "file_download_manifest_http/waits.rs"]
mod waits;
use source_fixture::{authority, id, otp, PASSWORD};
