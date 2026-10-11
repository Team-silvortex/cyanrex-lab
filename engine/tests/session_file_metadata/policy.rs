use super::{fixture::*, support::*, *};
use cyanrex_engine::services::collaboration_identity_store::IdentityStoreError;

async fn denied(f: &Fixture, token: &str, expected: IdentityStoreError) {
    let before = snapshot(f).await;
    let read = get_upload(f, token, 99).await;
    let published = get_publication(f, token, 99).await;
    let write = reserve(f, token, 99, &manifest(17, 99)).await;
    let after = snapshot(f).await;
    assert_eq!(
        read,
        Err(SessionFileMetadataError::Session(
            SessionCommandError::Identity(expected)
        ))
    );
    assert_eq!(
        published,
        Err(SessionFileMetadataError::Session(
            SessionCommandError::Identity(expected)
        ))
    );
    assert_eq!(
        write,
        Err(SessionFileMetadataError::Session(
            SessionCommandError::Identity(expected)
        ))
    );
    assert_eq!(after, before);
}

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_metadata_requires_current_audited_active_human_membership() {
    let f = Fixture::ready().await;
    denied(&f, &f.auth.target_token, IdentityStoreError::AccessDenied).await;
    let target = f.auth.bind_target().await;
    denied(&f, &f.auth.target_token, IdentityStoreError::AccessDenied).await;
    f.auth
        .source
        .apply_session_policy_command(
            &f.auth.manager_token,
            &SessionPolicyCommand {
                command_id: id(),
                expected_revision: None,
                desired: LegacyAccessPolicy {
                    membership: Membership {
                        workspace: scope(),
                        principal_id: target.binding.principal_id,
                        role_refs: vec![],
                        status: MembershipStatus::Active,
                    },
                    deployment_granted: false,
                },
            },
        )
        .await
        .unwrap();
    reserve(&f, &f.auth.target_token, 1, &manifest(17, 1))
        .await
        .unwrap();
    f.cleanup().await;
    // Raw alterations deliberately lack a matching audit head; they must not become permission.
    for (bad,restore,expected) in [
        ("UPDATE collaboration_workspaces SET status='archived'","UPDATE collaboration_workspaces SET status='active'",IdentityStoreError::ScopeInactive),
        ("UPDATE collaboration_identity_schema SET version=1","UPDATE collaboration_identity_schema SET version=2",IdentityStoreError::IdentityAuditNotEnabled),
        ("UPDATE collaboration_access_schema SET version=1","UPDATE collaboration_access_schema SET version=2",IdentityStoreError::AuditNotEnabled),
        ("UPDATE collaboration_principals SET status='disabled' WHERE display_name='source-learner'","UPDATE collaboration_principals SET status='active' WHERE display_name='source-learner'",IdentityStoreError::InvalidRecord),
        ("UPDATE collaboration_principals SET kind='agent' WHERE display_name='source-learner'","UPDATE collaboration_principals SET kind='human' WHERE display_name='source-learner'",IdentityStoreError::InvalidRecord),
        ("UPDATE collaboration_memberships SET role_refs=ARRAY['cyanrex.workspace.owner']::text[] WHERE principal_id=(SELECT principal_id FROM collaboration_legacy_identities WHERE username='source-learner')",
         "UPDATE collaboration_memberships SET role_refs=ARRAY[]::text[] WHERE principal_id=(SELECT principal_id FROM collaboration_legacy_identities WHERE username='source-learner')",IdentityStoreError::InvalidRecord),
    ] {
        let f=Fixture::ready().await;
        query(bad).execute(&f.auth.base.pool).await.unwrap();
        denied(&f,&f.auth.learner_token,expected).await;
        query(restore).execute(&f.auth.base.pool).await.unwrap();
        reserve(&f,&f.auth.learner_token,2,&manifest(17,2)).await.unwrap();
        f.pool_restored().await;f.cleanup().await;
    }
}
