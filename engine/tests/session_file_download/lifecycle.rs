use crate::{fixture::Fixture, support::*, *};

#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_download_requires_current_audited_human_and_exact_recipient_generation() {
    for mode in [
        "logout",
        "expired",
        "suspended",
        "retired",
        "archived",
        "rawrole",
        "agent",
        "identityaudit",
        "policyaudit",
    ] {
        checked(Fixture::ready().await,move |f|Box::pin(async move {
            healthy(f).await;let target=f.sharing.recipient;
            let bound=auth(f).store.lookup_legacy_account(scope(),&auth(f).target.username,target.account_id()).await.unwrap().unwrap();
            let expected=match mode {
                "logout"=>{auth(f).source.logout(&auth(f).target_token).await.unwrap();session(SessionCommandError::InvalidSession)},
                "expired"=>{query("UPDATE sessions SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE token=$1")
                    .bind(source_fixture::token_hash(&auth(f).target_token)).execute(&auth(f).base.pool).await.unwrap();session(SessionCommandError::InvalidSession)},
                "suspended"=>{membership(f,target.principal().principal_id,MembershipStatus::Suspended).await;identity(IdentityStoreError::AccessDenied)},
                "retired"=>{retire(f,&bound).await;identity(IdentityStoreError::AccessDenied)},
                "archived"=>{sql(f,&format!("UPDATE {}.collaboration_workspaces SET status='archived'",auth(f).base.schema)).await;identity(IdentityStoreError::ScopeInactive)},
                "rawrole"=>{sql(f,&format!("UPDATE {}.collaboration_memberships SET role_refs=ARRAY['cyanrex.workspace.owner']::text[] WHERE principal_id='{}'",auth(f).base.schema,target.principal().principal_id)).await;identity(IdentityStoreError::InvalidRecord)},
                "agent"=>{sql(f,&format!("UPDATE {}.collaboration_principals SET kind='agent' WHERE principal_id='{}'",auth(f).base.schema,target.principal().principal_id)).await;identity(IdentityStoreError::InvalidRecord)},
                "identityaudit"=>{sql(f,&format!("UPDATE {}.collaboration_identity_schema SET version=1",auth(f).base.schema)).await;identity(IdentityStoreError::IdentityAuditNotEnabled)},
                _=>{sql(f,&format!("UPDATE {}.collaboration_access_schema SET version=1",auth(f).base.schema)).await;identity(IdentityStoreError::AuditNotEnabled)},
            };
            assert_eq!(read(f,&f.request()).await,Err(expected),"recipient {mode}");
            assert_eq!(read(f,&request(f,99)).await,Err(expected),"absence cannot bypass current recipient checks");
            match mode {
                "logout"=>{let login=auth(f).base.seed_session(&auth(f).target).await;assert_eq!(read_as(f,&login.token,&f.workspace,&f.request()).await.unwrap(),f.sharing.base.manifest);},
                "expired"=>{refresh_expiry(f).await;healthy(f).await;},
                "suspended"=>{membership(f,target.principal().principal_id,MembershipStatus::Active).await;healthy(f).await;},
                "retired"=>{
                    let username=auth(f).target.username.clone();query("DELETE FROM users WHERE username=$1").bind(username.as_str()).execute(&auth(f).base.pool).await.unwrap();
                    let created=auth(f).source.register(&username,PASSWORD).await.unwrap();
                    let token=auth(f).source.login(&username,PASSWORD,&otp(&created.bootstrap.secret)).await.unwrap().token;
                    let fresh=auth(f).source.bind_session_account(&auth(f).manager_token,&SessionBindCommand {command_id:id(),workspace:scope(),target:created.account}).await.unwrap().entry.after;
                    auth(f).source.apply_session_policy_command(&auth(f).manager_token,&SessionPolicyCommand {command_id:id(),expected_revision:None,
                        desired:LegacyAccessPolicy {membership:Membership {workspace:scope(),principal_id:fresh.binding.principal_id,role_refs:vec![],status:MembershipStatus::Active},deployment_granted:false}}).await.unwrap();
                    let recipient=FileAccountRef::new(fresh.principal.reference,fresh.account_id);assert_ne!(recipient,target);
                    assert_eq!(read_as(f,&token,&f.workspace,&f.request()).await,Err(SessionFileDownloadError::PermissionDenied));
                    issue(f,2,f.sharing.actor(),f.sharing.actor(),recipient,f.sharing.expected_file.clone(),grant_fixture::validity()).await;
                    assert_eq!(read_as(f,&token,&f.workspace,&request(f,2)).await.unwrap(),f.sharing.base.manifest);
                },
                _=>{let restore=match mode {
                    "archived"=>format!("UPDATE {}.collaboration_workspaces SET status='active'",auth(f).base.schema),
                    "rawrole"=>format!("UPDATE {}.collaboration_memberships SET role_refs=ARRAY[]::text[] WHERE principal_id='{}'",auth(f).base.schema,target.principal().principal_id),
                    "agent"=>format!("UPDATE {}.collaboration_principals SET kind='human' WHERE principal_id='{}'",auth(f).base.schema,target.principal().principal_id),
                    "identityaudit"=>format!("UPDATE {}.collaboration_identity_schema SET version=2",auth(f).base.schema),
                    _=>format!("UPDATE {}.collaboration_access_schema SET version=2",auth(f).base.schema),
                };sql(f,&restore).await;healthy(f).await;}
            }drain(f).await;
        })).await;
    }
}
