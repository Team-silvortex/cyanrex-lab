//! Current-Session own recipient metadata. Not a download, management or publication check.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionFileRecipientGrantError,
            SessionFileRecipientGrantWorkspace, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, LegacyAccessPolicy, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        file_recipient_grant_store::{
            FileRecipientGrantStore, FileRecipientGrantStoreError, StoredFileRecipientGrant,
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
#[path = "session_file_recipient_grant/behavior.rs"]
mod behavior;
#[path = "session_file_recipient_grant/config.rs"]
mod config;
#[path = "session_file_recipient_grant/defaults.rs"]
mod defaults;
#[path = "session_file_recipient_grant/fixture.rs"]
mod fixture;
#[path = "session_file_recipient_grant/isolation.rs"]
mod isolation;
#[allow(dead_code)]
#[path = "file_recipient_grant/fixture.rs"]
mod ledger_fixture;
#[path = "session_file_recipient_grant/lifecycle.rs"]
mod lifecycle;
#[path = "session_file_recipient_grant/records.rs"]
mod records;
#[path = "session_file_recipient_grant/support.rs"]
mod support;
#[path = "session_file_recipient_grant/waits.rs"]
mod waits;

mod baseline {
    use super::{fixture::*, *};
    #[tokio::test]
    #[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_session_recipient_grant_reads_only_exact_current_recipient_metadata() {
        let f = Fixture::ready().await;
        let expected = f.issue(1, f.recipient()).await;
        let before = f.snapshot().await;
        // Only the Auth source transaction may perform this Session read; a standalone pool is
        // explicitly unavailable. There is no filesystem root, Published row or download permit.
        f.pool.close().await;
        let result = f
            .auth
            .source
            .get_session_file_recipient_grant(
                &f.auth.learner_token,
                &f.workspace,
                ledger_fixture::grant(1),
            )
            .await;
        let after = f.snapshot().await;
        let current = f.auth.current(f.recipient().principal().principal_id).await;
        f.cleanup().await;
        assert!(current.policy.membership.role_refs.is_empty());
        assert!(!current.policy.deployment_granted);
        assert_eq!(
            result.expect(
                "a current role-free audited active Human must read its exact recipient metadata"
            ),
            Some(expected.clone())
        );
        assert_eq!(
            expected.grant().file(),
            &ledger_fixture::file_for(scope(), 1)
        );
        assert_eq!(
            after, before,
            "own metadata read must not mutate any Auth, Registry, ledger row or relation OID"
        );
    }
}
