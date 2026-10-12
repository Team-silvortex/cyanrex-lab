//! Exact explicitly configured current-Session manager, not teacher-role or supplied attribution.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionFileGrantManagementWorkspace,
            SessionFilePublisherGrantError, SessionFilePublisherGrantWorkspace,
            SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            LegacyIdentityAction, LegacyIdentityCommand, StoredLegacyAccessPolicy,
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
#[path = "session_file_grant_management/actor_lifecycle.rs"]
mod actor_lifecycle;
#[path = "session_file_grant_management/authorization.rs"]
mod authorization;
#[path = "session_file_grant_management/behavior.rs"]
mod behavior;
#[path = "session_file_grant_management/concurrency.rs"]
mod concurrency;
#[path = "session_file_grant_management/defaults.rs"]
mod defaults;
#[path = "session_file_grant_management/faults.rs"]
mod faults;
#[path = "session_file_grant_management/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "session_file_publisher_grant/fixture.rs"]
mod publisher_fixture;
#[path = "session_file_grant_management/relations.rs"]
mod relations;
#[path = "session_file_grant_management/schema.rs"]
mod schema;
#[path = "session_file_grant_management/support.rs"]
mod support;
#[path = "session_file_grant_management/targets.rs"]
mod targets;
#[path = "session_file_grant_management/wait_logout.rs"]
mod wait_logout;
#[path = "session_file_grant_management/waits.rs"]
mod waits;

mod baseline {
    use super::{publisher_fixture::*, *};

    #[tokio::test]
    #[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_session_explicit_role_free_manager_issues_with_derived_attribution() {
        let f = fixture::Fixture::ready().await;
        let manager = f.manager();
        let subject = f.target();
        let policy = f.base.auth.current(manager.principal().principal_id).await;
        let before = f.base.snapshot().await;
        let issued = f
            .base
            .auth
            .source
            .issue_session_file_publisher_grant(
                &f.base.auth.learner_token,
                &f.management,
                grant(1),
                subject,
                validity(),
            )
            .await;
        let observed = f
            .base
            .auth
            .source
            .get_session_file_publisher_grant(
                &f.base.auth.manager_token,
                &f.base.workspace,
                grant(1),
            )
            .await;
        let after = f.base.snapshot().await;
        let source_prefix = format!("{}.", f.base.auth.base.schema);
        let head_name = format!("{}.{HEADS}", f.base.schema);
        let changes_name = format!("{}.{CHANGES}", f.base.schema);
        f.cleanup().await;
        assert!(policy.policy.membership.role_refs.is_empty());
        assert!(!policy.policy.deployment_granted);
        let record = issued
            .expect("the configured current audited role-free Human manager must commit issuance");
        assert_eq!(record.issuer(), manager);
        assert_eq!(record.grant().subject(), subject);
        assert_eq!(record.grant().id(), grant(1));
        assert_eq!(record.grant().workspace(), scope());
        assert_eq!(
            record.grant().revision(),
            RevisionNumber::try_from(1).unwrap()
        );
        assert_eq!(record.grant().state(), FileGrantState::Active);
        assert_eq!(record.grant().validity(), validity());
        assert_eq!(observed.unwrap(), Some(record));
        assert_eq!(before.0.len(), after.0.len());
        for (name, oid, rows) in &before.0 {
            let (_, new_oid, new_rows) = after.0.iter().find(|(n, _, _)| n == name).unwrap();
            assert_eq!(new_oid, oid);
            if name == &head_name || name == &changes_name {
                assert_eq!(new_rows.len(), rows.len() + 1);
                for row in rows {
                    assert!(new_rows.contains(row));
                }
            } else {
                assert_eq!(
                    new_rows, rows,
                    "all other source/Registry/marker rows remain exact"
                );
            }
            if name.starts_with(&source_prefix) {
                assert_eq!(new_rows, rows);
            }
        }
    }
}
