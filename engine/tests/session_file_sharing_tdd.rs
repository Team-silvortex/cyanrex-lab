#![cfg(unix)]
//! Current owner sharing of one trusted Published version; no downloading or byte IO command.
use chrono::Utc;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        auth_service::durable_source::{
            DurableAccountRef, DurableAuthError, DurableAuthSource, SessionBindCommand,
            SessionCommandError, SessionFilePublisherGrantWorkspace,
            SessionFilePublishingWorkspace, SessionFileRecipientGrantIssue,
            SessionFileRecipientGrantWorkspace, SessionFileSharingError,
            SessionFileSharingWorkspace, SessionPolicyCommand,
        },
        collaboration_identity_store::{
            CollaborationIdentityStore, IdentityStoreError, LegacyAccessPolicy,
            LegacyIdentityAction, LegacyIdentityCommand, StoredLegacyAccessPolicy,
            StoredLegacyIdentity,
        },
        file_publisher_grant_store::{
            FilePublisherGrantStore, FilePublisherGrantStoreError, StoredFilePublisherGrant,
        },
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
#[path = "session_file_sharing/behavior.rs"]
mod behavior;
#[allow(dead_code)]
#[path = "file_upload_staging/bytes.rs"]
mod bytes;
#[path = "session_file_sharing/concurrency.rs"]
mod concurrency;
#[path = "session_file_sharing/defaults.rs"]
mod defaults;
#[path = "session_file_sharing/faults.rs"]
mod faults;
#[path = "session_file_sharing/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "session_file_publisher_grant/fixture.rs"]
mod grant_fixture;
#[path = "session_file_sharing/grants.rs"]
mod grants;
#[path = "session_file_sharing/lifecycle.rs"]
mod lifecycle;
#[allow(dead_code)]
#[path = "session_file_publication/fixture.rs"]
mod publication_fixture;
#[path = "session_file_sharing/relations.rs"]
mod relations;
#[path = "session_file_sharing/schema.rs"]
mod schema;
#[path = "session_file_sharing/selectors.rs"]
mod selectors;
#[path = "session_file_sharing/support.rs"]
mod support;
#[path = "session_file_sharing/waits.rs"]
mod waits;

mod baseline {
    use super::{fixture::*, *};

    #[tokio::test]
    #[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
    async fn postgres_session_publisher_shares_exact_published_version_and_revokes_after_publisher_revocation(
    ) {
        let f = Fixture::ready().await;
        let actor = f.actor();
        let recipient = f.recipient;
        let actor_policy = f
            .base
            .grants
            .auth
            .current(actor.principal().principal_id)
            .await;
        let recipient_policy = f
            .base
            .grants
            .auth
            .current(recipient.principal().principal_id)
            .await;
        let command = f.command(1);
        let before = f.snapshot().await;
        let issued: Result<StoredFileRecipientGrant, _> = f
            .base
            .grants
            .auth
            .source
            .issue_session_file_recipient_grant(
                &f.base.grants.auth.learner_token,
                &f.workspace,
                &command,
            )
            .await;
        let after_issue = f.snapshot().await;
        let visible = f
            .base
            .grants
            .auth
            .source
            .get_session_file_recipient_grant(
                &f.base.grants.auth.target_token,
                &f.recipient_workspace,
                recipient_grant(1),
            )
            .await;
        let trusted = f.store.get(recipient_grant(1), recipient).await;
        let after_get = f.snapshot().await;
        let (reopened, pool) = f.reopen().await;
        let independent = reopened.get(recipient_grant(1), recipient).await;
        let after_reopen = f.snapshot().await;
        let publisher_revoked = f
            .base
            .grants
            .store
            .revoke(grant_fixture::grant(1), f.base.grants.issuer(), revision(1))
            .await;
        let after_publisher_revoked = f.snapshot().await;
        let revoked = f
            .base
            .grants
            .auth
            .source
            .revoke_session_file_recipient_grant(
                &f.base.grants.auth.learner_token,
                &f.workspace,
                recipient_grant(1),
                revision(1),
            )
            .await;
        let after_revoke = f.snapshot().await;
        let independent_revoked = reopened.get(recipient_grant(1), recipient).await;
        let after_final_get = f.snapshot().await;
        let history=query(&format!("SELECT revision,actor_id,actor_account_id,grant_payload FROM {}.{CHANGES} WHERE grant_id=$1 ORDER BY revision",f.schema)).bind(recipient_grant(1).as_uuid()).fetch_all(&f.base.grants.auth.base.admin).await.unwrap();
        let final_snapshot = f.snapshot().await;
        pool.close().await;
        let expected_file = f.expected_file.clone();
        let schema = f.schema.clone();
        f.cleanup().await;

        assert!(actor_policy.policy.membership.role_refs.is_empty());
        assert!(!actor_policy.policy.deployment_granted);
        assert!(recipient_policy.policy.membership.role_refs.is_empty());
        assert_eq!(
            recipient_policy.policy.membership.status,
            MembershipStatus::Active
        );
        let original=issued.expect("a current role-free Published owner with its exact active publisher grant must commit exact recipient issuance");
        assert_eq!(original.issuer(), actor);
        assert_eq!(original.grant().id(), recipient_grant(1));
        assert_eq!(original.grant().revision(), revision(1));
        assert_eq!(original.grant().state(), FileGrantState::Active);
        assert_eq!(original.grant().file(), &expected_file);
        assert_eq!(original.grant().publisher(), actor);
        assert_eq!(original.grant().recipient(), recipient);
        assert_eq!(original.grant().validity(), command.validity);
        assert_eq!(visible.unwrap(), Some(original.clone()));
        assert_eq!(trusted.unwrap(), Some(original.clone()));
        assert_eq!(independent.unwrap(), Some(original.clone()));
        assert_eq!(after_get, after_issue);
        assert_eq!(after_reopen, after_issue);
        recipient_delta(&schema, &before, &after_issue, 1, false);
        let permission = publisher_revoked
            .expect("explicit fixture revocation of publisher permission must commit");
        assert_eq!(permission.grant().state(), FileGrantState::Revoked);
        assert_eq!(after_publisher_revoked.recipient, after_issue.recipient);
        assert_eq!(after_publisher_revoked.base.root, after_issue.base.root);
        assert_eq!(after_publisher_revoked.base.files, after_issue.base.files);
        publisher_revocation_delta(&after_issue, &after_publisher_revoked);
        let retired=revoked.expect("the same current original publisher must be able to stop its old share after publisher permission was revoked");
        assert_eq!(retired.issuer(), actor);
        assert_eq!(retired.grant().revision(), revision(2));
        assert_eq!(retired.grant().state(), FileGrantState::Revoked);
        assert_eq!(retired.grant().file(), original.grant().file());
        assert_eq!(retired.grant().publisher(), actor);
        assert_eq!(retired.grant().recipient(), recipient);
        assert_eq!(retired.grant().validity(), original.grant().validity());
        recipient_delta(&schema, &after_publisher_revoked, &after_revoke, 1, true);
        assert_eq!(independent_revoked.unwrap(), Some(retired.clone()));
        assert_eq!(after_final_get, after_revoke);
        assert_eq!(final_snapshot, after_revoke);
        assert_eq!(history.len(), 2);
        for (index, record) in [original, retired].into_iter().enumerate() {
            let row = &history[index];
            assert_eq!(row.get::<i64, _>("revision"), index as i64 + 1);
            assert_eq!(
                row.get::<Uuid, _>("actor_id"),
                actor.principal().principal_id.as_uuid()
            );
            assert_eq!(
                row.get::<Uuid, _>("actor_account_id"),
                actor.account_id().as_uuid()
            );
            assert_eq!(
                row.get::<Vec<u8>, _>("grant_payload"),
                serde_json::to_vec(record.grant()).unwrap()
            );
        }
    }
}
