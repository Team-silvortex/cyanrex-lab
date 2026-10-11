#![cfg(unix)]
//! Session-derived owner/account metadata only; not publication, byte health or recipient authority.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionFileMetadataError, SessionFilePublicationWorkspace,
            SessionFileUploadWorkspace, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        file_upload_publication::{
            FileUploadPublicationRecord, FileUploadPublicationStore,
            FileUploadPublicationStoreError,
        },
        file_upload_store::{
            FileUploadLimits, FileUploadReservation, FileUploadStore, FileUploadStoreError,
        },
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
#[path = "session_file_metadata/mod.rs"]
mod cases;

mod baseline {
    use super::cases::fixture::*;
    #[tokio::test]
    #[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_session_upload_reservation_commits_and_gets_context_owner() {
        let f = Fixture::ready().await;
        let before = f.rows(&f.auth.base.schema).await;
        let manifest = manifest(17, 1);
        let result = f
            .auth
            .source
            .reserve_session_file_upload(
                &f.auth.learner_token,
                &f.upload_workspace,
                upload(1),
                &manifest,
            )
            .await;
        let observed = f
            .auth
            .source
            .get_session_file_upload(&f.auth.learner_token, &f.upload_workspace, upload(1))
            .await;
        let after = f.rows(&f.auth.base.schema).await;
        let expected_owner = f.auth.learner.principal.reference;
        let expected_account = f.auth.learner.account_id;
        let stored = f
            .uploads
            .get(upload(1), expected_owner, expected_account)
            .await
            .unwrap();
        f.cleanup().await;
        let reservation=result.expect("current active audited Human/member must reserve its own upload in the source transaction");
        assert_eq!(reservation.upload_id(), upload(1));
        assert_eq!(reservation.owner(), expected_owner);
        assert_eq!(reservation.account_id(), expected_account);
        assert_eq!(reservation.manifest(), &manifest);
        assert_eq!(observed.unwrap(), Some(reservation.clone()));
        assert_eq!(stored, Some(reservation));
        assert_eq!(
            after, before,
            "reserve/get must not mutate source sessions or identity/policy audit rows"
        );
    }
}
