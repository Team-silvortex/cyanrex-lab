use super::fixture::*;
use cyanrex_engine::services::collaboration_identity_store::{
    CollaborationIdentityStore, LegacyPolicyCommand,
};
use sqlx_core::query::query;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_current_private_owner_and_account_incarnation_remain_authority(
) {
    for mode in ["unbound", "revoked", "expired", "suspended", "recreated"] {
        Fixture::checked(|f| Box::pin(async move {
            let p=Prepared::new(f).await;
            let registry=CollaborationIdentityStore::new(f.owner().clone());
            let current=registry.legacy_access(f.config().scope(),p.owners[1].principal_id).await.unwrap().unwrap();
            let mut desired=current.policy; desired.membership.role_refs.clear(); desired.deployment_granted=false;
            registry.apply_legacy_policy_command(&LegacyPolicyCommand { command_id:uuid::Uuid::new_v4().try_into().unwrap(),
                actor:p.owners[0],expected_revision:Some(current.revision),desired }).await.unwrap();
            let connector=p.connector().await;
            let created=p.create(&connector,1,1,"普通协作者的私人讲义，不需要教师或部署权限").await.unwrap();
            assert_eq!(created.owner,p.owners[1]); p.counts(1).await;
            match mode {
                "revoked"=>f.sql(&format!("DELETE FROM {}.sessions WHERE token='{}'",f.source,token_hash(&p.tokens[1]))).await,
                "expired"=>f.sql(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token='{}'",f.source,token_hash(&p.tokens[1]))).await,
                "suspended"=>p.suspend(1).await,
                "recreated"=>{
                    f.sql(&format!("DELETE FROM {}.users WHERE username='index-owner-1'",f.source)).await;
                    let replacement=uuid::Uuid::new_v4();
                    within(query(&format!("INSERT INTO {}.users(username,password_salt,password_hash,totp_secret,account_id) VALUES('index-owner-1','synthetic','noncredential','nonenrollment',$1)",f.source))
                        .bind(replacement).execute(f.owner())).await.unwrap();
                    within(query(&format!("INSERT INTO {}.sessions(token,username,account_id,expires_at) VALUES($1,'index-owner-1',$2,clock_timestamp()+interval '12 hours')",f.source))
                        .bind(token_hash(&p.tokens[1])).bind(replacement).execute(f.owner())).await.unwrap();
                },_=>{}
            }
            let before=snapshot(f).await; let files=p.files();
            let denied_owner=if mode=="unbound" {2}else{1};
            assert!(p.create(&connector,denied_owner,2,"No inherited owner").await.is_err(),"{mode}");
            assert_eq!(before,snapshot(f).await); assert_eq!(files,p.files()); p.read_committed(&created,"普通协作者的私人讲义，不需要教师或部署权限".as_bytes()).await;
            drop(connector); assert!(super::storage_helpers::no_connections(f).await); assert!(p.finish());
        })).await;
    }
}
