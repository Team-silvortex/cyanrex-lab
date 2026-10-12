#![cfg(unix)]
//! One current recipient observes an exact Published manifest, never bytes or an access token.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionFileDownloadError, SessionFileDownloadRequest,
            SessionFileDownloadWorkspace, SessionFilePublisherGrantWorkspace,
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
#[path = "session_file_download/baseline.rs"]
mod baseline;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "session_file_download/cancellation.rs"]
mod cancellation;
#[path = "session_file_download/configuration.rs"]
mod configuration;
#[path = "session_file_download/defaults.rs"]
mod defaults;
#[path = "session_file_download/fences.rs"]
mod fences;
#[path = "session_file_download/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "session_file_publisher_grant/fixture.rs"]
mod grant_fixture;
#[path = "session_file_download/grants.rs"]
mod grants;
#[path = "session_file_download/lifecycle.rs"]
mod lifecycle;
#[allow(dead_code)]
#[path = "session_file_publication/fixture.rs"]
mod publication_fixture;
#[path = "session_file_download/records.rs"]
mod records;
#[path = "session_file_download/relations.rs"]
mod relations;
#[path = "session_file_download/selectors.rs"]
mod selectors;
#[allow(dead_code)]
#[path = "session_file_sharing/fixture.rs"]
mod sharing_fixture;
#[path = "session_file_download/support.rs"]
mod support;
#[path = "session_file_download/waits.rs"]
mod waits;
