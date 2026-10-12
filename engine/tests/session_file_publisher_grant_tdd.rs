//! Current-Session own publisher-grant metadata, not issuance or permission to publish bytes.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionFilePublisherGrantError,
            SessionFilePublisherGrantWorkspace, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        file_publisher_grant_store::{
            FilePublisherGrantStore, FilePublisherGrantStoreError, StoredFilePublisherGrant,
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
#[path = "session_file_publisher_grant/behavior.rs"]
mod behavior;
#[path = "session_file_publisher_grant/config.rs"]
mod config;
#[path = "session_file_publisher_grant/defaults.rs"]
mod defaults;
#[path = "session_file_publisher_grant/faults.rs"]
mod faults;
#[allow(dead_code)]
#[path = "session_file_publisher_grant/fixture.rs"]
mod fixture;
#[path = "session_file_publisher_grant/lifecycle.rs"]
mod lifecycle;
#[path = "session_file_publisher_grant/support.rs"]
mod support;
#[path = "session_file_publisher_grant/transactions.rs"]
mod transactions;
#[path = "session_file_publisher_grant/waits.rs"]
mod waits;

mod baseline {
    use super::fixture::*;

    #[tokio::test]
    #[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_session_publisher_grant_reads_exact_current_subject_metadata() {
        let f = Fixture::ready().await;
        let expected = f.issue(1, f.subject()).await;
        let before = f.snapshot().await;
        let result = f
            .auth
            .source
            .get_session_file_publisher_grant(&f.auth.learner_token, &f.workspace, grant(1))
            .await;
        let after = f.snapshot().await;
        let subject = f.subject();
        f.cleanup().await;
        let observed = result
            .expect("a current audited active Human member must read its own exact grant metadata");
        assert_eq!(observed, Some(expected.clone()));
        assert_eq!(expected.grant().subject(), subject);
        assert_eq!(
            after, before,
            "metadata reads must not mutate any source, Registry or grant row/OID"
        );
    }
}
