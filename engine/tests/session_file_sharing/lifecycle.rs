use super::support::*;
use crate::{fixture::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_sharing_rechecks_current_actor_and_target_but_revokes_retired_recipients()
{
    for mode in ["suspended", "retired", "deleted", "agent", "policy"] {
        checked(Fixture::ready().await,|f|Box::pin(async move{
            let old=issue(f,1).await.unwrap();let expected=match mode{
                "suspended"=>{membership(f,f.recipient.principal().principal_id,MembershipStatus::Suspended).await;identity(IdentityStoreError::AccessDenied)},
                "retired"=>{let bound=f.base.grants.auth.store.lookup_legacy_account(scope(),&f.base.grants.auth.target.username,f.recipient.account_id()).await.unwrap().unwrap();retire(f,&bound).await;identity(IdentityStoreError::AccessDenied)},
                "deleted"=>{sql(f,&format!("DELETE FROM {}.users WHERE account_id='{}'",f.base.grants.auth.base.schema,f.recipient.account_id())).await;session(SessionCommandError::AccountMismatch)},
                "agent"=>{sql(f,&format!("UPDATE {}.collaboration_principals SET kind='agent' WHERE principal_id='{}'",f.base.grants.auth.base.schema,f.recipient.principal().principal_id)).await;identity(IdentityStoreError::InvalidRecord)},
                _=>{sql(f,&format!("UPDATE {}.collaboration_memberships SET role_refs=ARRAY['cyanrex.workspace.owner']::text[] WHERE principal_id='{}'",f.base.grants.auth.base.schema,f.recipient.principal().principal_id)).await;identity(IdentityStoreError::InvalidRecord)},
            };assert_eq!(issue(f,2).await,Err(expected),"recipient {mode}");
            if mode=="agent"{sql(f,&format!("UPDATE {}.collaboration_principals SET kind='human' WHERE principal_id='{}'",f.base.grants.auth.base.schema,f.recipient.principal().principal_id)).await;issue(f,2).await.unwrap();}
            if mode=="policy"{sql(f,&format!("UPDATE {}.collaboration_memberships SET role_refs=ARRAY[]::text[] WHERE principal_id='{}'",f.base.grants.auth.base.schema,f.recipient.principal().principal_id)).await;issue(f,2).await.unwrap();}
            let stopped=revoke(f,1,revision(1)).await.unwrap();assert_eq!(stopped.issuer(),old.issuer());assert_eq!(stopped.grant().recipient(),f.recipient);
        })).await;
    }
    for mode in [
        "logout",
        "expired",
        "suspended",
        "retired",
        "agent",
        "audit",
    ] {
        checked(Fixture::ready().await,|f|Box::pin(async move{
            issue(f,1).await.unwrap();let expected=match mode{
                "logout"=>{f.base.grants.auth.source.logout(&f.base.grants.auth.learner_token).await.unwrap();session(SessionCommandError::InvalidSession)},
                "expired"=>{sql(f,&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-INTERVAL '1 second' WHERE token='{}'",f.base.grants.auth.base.schema,source_fixture::token_hash(&f.base.grants.auth.learner_token))).await;session(SessionCommandError::InvalidSession)},
                "suspended"=>{membership(f,f.actor().principal().principal_id,MembershipStatus::Suspended).await;identity(IdentityStoreError::AccessDenied)},
                "retired"=>{retire(f,&f.base.grants.auth.learner).await;identity(IdentityStoreError::AccessDenied)},
                "agent"=>{sql(f,&format!("UPDATE {}.collaboration_principals SET kind='agent' WHERE principal_id='{}'",f.base.grants.auth.base.schema,f.actor().principal().principal_id)).await;identity(IdentityStoreError::InvalidRecord)},
                _=>{sql(f,&format!("UPDATE {}.collaboration_access_schema SET version=1",f.base.grants.auth.base.schema)).await;identity(IdentityStoreError::AuditNotEnabled)},
            };assert_eq!(issue(f,2).await,Err(expected),"actor {mode}");assert_eq!(revoke(f,1,revision(1)).await,Err(expected),"current owner still required for revocation");
            if mode=="suspended"{membership(f,f.actor().principal().principal_id,MembershipStatus::Active).await;issue(f,2).await.unwrap();revoke(f,1,revision(1)).await.unwrap();}
            if mode=="agent"{sql(f,&format!("UPDATE {}.collaboration_principals SET kind='human' WHERE principal_id='{}'",f.base.grants.auth.base.schema,f.actor().principal().principal_id)).await;issue(f,2).await.unwrap();}
            if mode=="audit"{sql(f,&format!("UPDATE {}.collaboration_access_schema SET version=2",f.base.grants.auth.base.schema)).await;issue(f,2).await.unwrap();}
            if mode=="retired"{
                let username=&f.base.grants.auth.learner.binding.username;sql(f,&format!("DELETE FROM {}.users WHERE username='{}'",f.base.grants.auth.base.schema,username.as_str())).await;
                let created=f.base.grants.auth.source.register(username,PASSWORD).await.unwrap();let token=f.base.grants.auth.source.login(username,PASSWORD,&otp(&created.bootstrap.secret)).await.unwrap().token;
                let bound=f.base.grants.auth.source.bind_session_account(&f.base.grants.auth.manager_token,&SessionBindCommand{command_id:id(),workspace:scope(),target:created.account}).await.unwrap().entry.after;
                f.base.grants.auth.source.apply_session_policy_command(&f.base.grants.auth.manager_token,&SessionPolicyCommand{command_id:id(),expected_revision:None,desired:LegacyAccessPolicy{membership:Membership{workspace:scope(),principal_id:bound.binding.principal_id,role_refs:vec![],status:MembershipStatus::Active},deployment_granted:false}}).await.unwrap();
                let fresh=FileAccountRef::new(bound.principal.reference,bound.account_id);assert_ne!(fresh,f.actor());f.base.grants.store.issue(grant_fixture::grant(2),f.base.grants.issuer(),fresh,grant_fixture::validity()).await.unwrap();let mut c=f.command(2);c.publisher_grant_id=grant_fixture::grant(2);assert_eq!(issue_as(f,&token,&f.workspace,&c).await,Err(SessionFileSharingError::PermissionDenied));assert_eq!(revoke_as(f,&token,1,revision(1)).await,Err(SessionFileSharingError::PermissionDenied));
            }
        })).await;
    }
}
