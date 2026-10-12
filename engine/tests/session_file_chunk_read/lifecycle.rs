use crate::{fixture::Fixture, support::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_chunk_read_rechecks_current_human_membership_session_and_account_generation() {
    for mode in [
        "logout",
        "expired",
        "suspended",
        "retired",
        "agent",
        "audit",
    ] {
        checked(Fixture::ready(1).await,move |f|Box::pin(async move {
            healthy(f).await;let target=f.sharing.recipient;
            let bound=auth(f).store.lookup_legacy_account(scope(),&auth(f).target.username,target.account_id()).await.unwrap().unwrap();
            let error=match mode {
                "logout"=>{auth(f).source.logout(&auth(f).target_token).await.unwrap();session(SessionCommandError::InvalidSession)},
                "expired"=>{query("UPDATE sessions SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE token=$1").bind(source_fixture::token_hash(&auth(f).target_token)).execute(&auth(f).base.pool).await.unwrap();session(SessionCommandError::InvalidSession)},
                "suspended"=>{membership(f,target.principal().principal_id,MembershipStatus::Suspended).await;identity(IdentityStoreError::AccessDenied)},
                "retired"=>{retire(f,&bound).await;identity(IdentityStoreError::AccessDenied)},
                "agent"=>{sql(f,&format!("UPDATE {}.collaboration_principals SET kind='agent' WHERE principal_id='{}'",auth(f).base.schema,target.principal().principal_id)).await;identity(IdentityStoreError::InvalidRecord)},
                _=>{sql(f,&format!("UPDATE {}.collaboration_access_schema SET version=1",auth(f).base.schema)).await;identity(IdentityStoreError::AuditNotEnabled)},
            };
            assert_eq!(read(f,&f.request()).await,Err(error));assert_eq!(read(f,&request(f,99)).await,Err(error));
            match mode {
                "logout"=>{let login=auth(f).base.seed_session(&auth(f).target).await;assert_eq!(read_as(f,&auth(f).source,&login.token,&f.workspace,&f.request()).await.unwrap(),expected(f.sharing.expected_file.clone(),0,17));},
                "expired"=>{refresh_expiry(f).await;healthy(f).await;},
                "suspended"=>{membership(f,target.principal().principal_id,MembershipStatus::Active).await;healthy(f).await;},
                "retired"=>{
                    let username=auth(f).target.username.clone();query("DELETE FROM users WHERE username=$1").bind(username.as_str()).execute(&auth(f).base.pool).await.unwrap();
                    let created=auth(f).source.register(&username,PASSWORD).await.unwrap();let token=auth(f).source.login(&username,PASSWORD,&otp(&created.bootstrap.secret)).await.unwrap().token;
                    let fresh=auth(f).source.bind_session_account(&auth(f).manager_token,&SessionBindCommand {command_id:id(),workspace:scope(),target:created.account}).await.unwrap().entry.after;
                    auth(f).source.apply_session_policy_command(&auth(f).manager_token,&SessionPolicyCommand {command_id:id(),expected_revision:None,desired:LegacyAccessPolicy {membership:Membership {workspace:scope(),principal_id:fresh.binding.principal_id,role_refs:vec![],status:MembershipStatus::Active},deployment_granted:false}}).await.unwrap();
                    let target=FileAccountRef::new(fresh.principal.reference,fresh.account_id);assert_ne!(target,f.sharing.recipient);
                    assert_eq!(read_as(f,&auth(f).source,&token,&f.workspace,&f.request()).await,Err(denied()));
                    issue(f,2,f.sharing.actor(),target,f.sharing.expected_file.clone(),grant_fixture::validity()).await;
                    assert_eq!(read_as(f,&auth(f).source,&token,&f.workspace,&request(f,2)).await.unwrap(),expected(f.sharing.expected_file.clone(),0,17));
                },
                _=>{let restore=if mode=="agent" {format!("UPDATE {}.collaboration_principals SET kind='human' WHERE principal_id='{}'",auth(f).base.schema,target.principal().principal_id)}
                    else{format!("UPDATE {}.collaboration_access_schema SET version=2",auth(f).base.schema)};sql(f,&restore).await;healthy(f).await;}
            }drain(f).await;
        })).await;
    }
}
