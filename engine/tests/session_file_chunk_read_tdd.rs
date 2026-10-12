#![cfg(unix)]
//! Current-recipient verified single-chunk bytes, not a whole-file proof or reusable grant.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionFileChunkRead, SessionFileChunkReadError,
            SessionFileChunkReadRequest, SessionFileChunkReadWorkspace, SessionFileDownloadError,
            SessionFileDownloadRequest, SessionFilePublisherGrantWorkspace,
            SessionFilePublishingWorkspace, SessionFileRecipientGrantIssue,
            SessionFileRecipientGrantWorkspace, SessionFileSharingWorkspace, SessionPolicyCommand,
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
        file_transfer::staging::ChunkStaging,
        file_upload_publication::{FileUploadPublicationStore, FileUploadPublicationStoreError},
    },
};
use sqlx_core::{query::query, row::Row};
use sqlx_postgres::{PgPool, PgPoolOptions};
use uuid::Uuid;

#[allow(dead_code)]
#[path = "durable_auth_source/fixture.rs"]
mod source_fixture;
use source_fixture::{authority, id, otp, PASSWORD};
#[allow(dead_code)]
#[path = "durable_collaboration/fixture.rs"]
mod identity_fixture;
use identity_fixture::scope;
#[path = "session_file_chunk_read/authorization.rs"]
mod authorization;
#[path = "session_file_chunk_read/baseline.rs"]
mod baseline;
#[path = "session_file_chunk_read/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "session_file_chunk_read/cancellation.rs"]
mod cancellation;
#[path = "session_file_chunk_read/configuration.rs"]
mod configuration;
#[path = "session_file_chunk_read/defaults.rs"]
mod defaults;
#[path = "session_file_chunk_read/filesystem.rs"]
mod filesystem;
#[path = "session_file_chunk_read/fixture.rs"]
mod fixture;
#[path = "session_file_chunk_read/gap.rs"]
mod gap;
#[allow(dead_code)]
#[path = "session_file_publisher_grant/fixture.rs"]
mod grant_fixture;
#[path = "session_file_chunk_read/lifecycle.rs"]
mod lifecycle;
#[path = "session_file_chunk_read/pool_gate.rs"]
mod pool_gate;
#[allow(dead_code)]
#[path = "session_file_publication/fixture.rs"]
mod publication_fixture;
#[path = "session_file_chunk_read/records.rs"]
mod records;
#[allow(dead_code)]
#[path = "session_file_sharing/fixture.rs"]
mod sharing_fixture;
#[path = "session_file_chunk_read/support.rs"]
mod support;
#[path = "session_file_chunk_read/waits.rs"]
mod waits;
