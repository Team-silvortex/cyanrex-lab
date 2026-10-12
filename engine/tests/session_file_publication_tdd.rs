#![cfg(unix)]
//! Current audited Session + exact active publisher grant + real bytes, not a detached permit.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionFilePublisherGrantWorkspace, SessionFilePublishingError,
            SessionFilePublishingWorkspace, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            LegacyIdentityAction, LegacyIdentityCommand, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        file_publisher_grant_store::{
            FilePublisherGrantStore, FilePublisherGrantStoreError, StoredFilePublisherGrant,
        },
        file_upload_staging::{
            FileUploadPublicationAttempt, FileUploadPublicationError, FileUploadPublicationReceipt,
            FileUploadPublicationState, FileUploadPublisher,
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
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "session_file_publication/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "session_file_publisher_grant/fixture.rs"]
mod grant_fixture;

#[path = "session_file_publication/behavior.rs"]
mod behavior;
#[path = "session_file_publication/cancellation.rs"]
mod cancellation;
#[path = "session_file_publication/concurrency.rs"]
mod concurrency;
#[path = "session_file_publication/defaults.rs"]
mod defaults;
#[path = "session_file_publication/faults.rs"]
mod faults;
#[path = "session_file_publication/finalization.rs"]
mod finalization;
#[path = "session_file_publication/grant_gap.rs"]
mod grant_gap;
#[path = "session_file_publication/grants.rs"]
mod grants;
#[path = "session_file_publication/lifecycle.rs"]
mod lifecycle;
#[path = "session_file_publication/pool_gate.rs"]
mod pool_gate;
#[path = "session_file_publication/relations.rs"]
mod relations;
#[path = "session_file_publication/support.rs"]
mod support;
#[path = "session_file_publication/waits.rs"]
mod waits;

mod baseline {
    use super::{fixture::*, *};
    use cyanrex_engine::services::file_upload_staging::FileUploadPublicationState;

    #[tokio::test]
    #[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_current_role_free_session_with_exact_active_grant_publishes_real_confirmed_bytes(
    ) {
        let f = Fixture::ready(17).await;
        let explicit = f.grants.issue(1, f.actor()).await;
        let before = f.snapshot().await;
        let mut attempt = f
            .publisher
            .prepare(upload(1), f.actor().principal(), f.actor().account_id())
            .await
            .unwrap();
        let prepared = f.snapshot().await;
        let result = f
            .grants
            .auth
            .source
            .publish_session_file_upload(
                &f.grants.auth.learner_token,
                &f.workspace,
                grant_fixture::grant(1),
                &mut attempt,
            )
            .await;
        let state = attempt.state();
        let observed = f
            .publication
            .get(upload(1), f.actor().principal(), f.actor().account_id())
            .await;
        let after = f.snapshot().await;
        let actor = f.actor();
        let manifest = f.manifest.clone();
        let pub_table = format!("{}.collaboration_file_publications", f.publication_schema);
        let current = f.grants.auth.current(actor.principal().principal_id).await;
        f.cleanup().await;
        assert!(current.policy.membership.role_refs.is_empty());
        assert!(!current.policy.deployment_granted);
        assert_eq!(explicit.grant().subject(), actor);
        assert_eq!(explicit.grant().state(), FileGrantState::Active);
        assert_eq!(
            prepared, before,
            "trusted prepare is metadata-only and changes no rows or files"
        );
        let receipt=result.expect("a current role-free audited Session with its exact active grant must acknowledge real publication");
        assert_eq!(state, FileUploadPublicationState::Confirmed);
        assert_eq!(receipt.upload_id(), upload(1));
        assert_eq!(receipt.owner(), actor.principal());
        assert_eq!(receipt.account_id(), actor.account_id());
        assert_eq!(receipt.manifest(), &manifest);
        let row = observed
            .unwrap()
            .expect("the new immutable publication must persist after the same-source commit");
        assert_eq!(row.upload_id(), upload(1));
        assert_eq!(row.owner(), actor.principal());
        assert_eq!(row.account_id(), actor.account_id());
        assert_eq!(row.manifest(), &manifest);
        assert_eq!(after.root, before.root);
        assert_eq!(after.files, before.files);
        assert_eq!(after.database.0.len(), before.database.0.len());
        for (name, oid, rows) in &before.database.0 {
            let (_, new_oid, new_rows) =
                after.database.0.iter().find(|(n, _, _)| n == name).unwrap();
            assert_eq!(new_oid, oid);
            if name == &pub_table {
                assert_eq!(new_rows.len(), rows.len() + 1);
                for r in rows {
                    assert!(new_rows.contains(r));
                }
            } else {
                assert_eq!(
                    new_rows, rows,
                    "source/Registry/grants/intent/journal/markers remain exact"
                );
            }
        }
    }
}
