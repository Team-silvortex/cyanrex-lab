use super::fixture::*;
use axum::{body::Body, http::StatusCode};
use cyanrex_engine::{
    models::collaboration::ArtifactRevision,
    services::collaboration_identity_store::{CollaborationIdentityStore, LegacyPolicyCommand},
};
use serde_json::json;
use sqlx_core::query::query;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_http_current_authority_and_duplicate_ids_are_private() {
    for mode in ["unbound", "revoked", "expired", "suspended", "recreated"] {
        Fixture::checked(|f|Box::pin(async move {
            let p=Prepared::new(f).await;
            let registry=CollaborationIdentityStore::new(f.owner().clone());
            let current=registry.legacy_access(f.config().scope(),p.owners[1].principal_id).await.unwrap().unwrap();
            let mut desired=current.policy;desired.membership.role_refs.clear();desired.deployment_granted=false;
            registry.apply_legacy_policy_command(&LegacyPolicyCommand{command_id:uuid::Uuid::new_v4().try_into().unwrap(),actor:p.owners[0],expected_revision:Some(current.revision),desired}).await.unwrap();
            let route=router(&p,2).await;let private=p.non_artifacts().await;
            let text="普通协作者的私人材料，不需要教师或部署权限";
            let created=call(&route,request(&p,1,"POST",PATH,wire(1,"普通协作者材料",text)),StatusCode::CREATED).await;
            let artifact:ArtifactRevision=serde_json::from_value(created["artifact"].clone()).unwrap();
            assert_eq!(artifact.owner,p.owners[1]);assert_eq!(artifact.reference,p.reference(1,text.as_bytes()));
            p.read_committed(&artifact,text.as_bytes()).await;assert_eq!(private,p.non_artifacts().await);
            let before=snapshot(f).await;let files=p.files();
            for owner in [0,1] {
                error(&route,request(&p,owner,"POST",PATH,wire(1,"Cannot replace owner or bytes","Different actual body")),StatusCode::CONFLICT,"conflict","unconfirmed").await;
            }
            let reused=json!({"schema_version":1,"artifact_id":artifact_id(2),"revision_id":revision_id(1),"title":"No revision reuse","text":"New actual request bytes"});
            error(&route,request(&p,1,"POST",PATH,Body::from(reused.to_string())),StatusCode::CONFLICT,"conflict","unconfirmed").await;
            assert_eq!(before,snapshot(f).await);assert_eq!(files,p.files());
            match mode {
                "revoked"=>f.sql(&format!("DELETE FROM {}.sessions WHERE token='{}'",f.source,token_hash(&p.tokens[1]))).await,
                "expired"=>f.sql(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token='{}'",f.source,token_hash(&p.tokens[1]))).await,
                "suspended"=>p.suspend(1).await,
                "recreated"=>{
                    f.sql(&format!("DELETE FROM {}.users WHERE username='index-owner-1'",f.source)).await;
                    let account=uuid::Uuid::new_v4();
                    within(query(&format!("INSERT INTO {}.users(username,password_salt,password_hash,totp_secret,account_id) VALUES('index-owner-1','synthetic','noncredential','nonenrollment',$1)",f.source)).bind(account).execute(f.owner())).await.unwrap();
                    within(query(&format!("INSERT INTO {}.sessions(token,username,account_id,expires_at) VALUES($1,'index-owner-1',$2,clock_timestamp()+interval '12 hours')",f.source)).bind(token_hash(&p.tokens[1])).bind(account).execute(f.owner())).await.unwrap();
                },_=>{}
            }
            let before=snapshot(f).await;let files=p.files();
            let (owner,status,code)=if mode=="unbound" {(2,StatusCode::FORBIDDEN,"access_denied")} else if mode=="suspended" {(1,StatusCode::FORBIDDEN,"access_denied")} else if mode=="recreated" {(1,StatusCode::FORBIDDEN,"access_denied")} else {(1,StatusCode::UNAUTHORIZED,"invalid_session")};
            error(&route,request(&p,owner,"POST",PATH,wire(3,"Denied new publication","A well-formed new body must reach current authorization")),status,code,"unconfirmed").await;
            assert_eq!(before,snapshot(f).await);assert_eq!(files,p.files());p.counts(1).await;
            p.read_committed(&artifact,text.as_bytes()).await;
            drop((route,registry));assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
        })).await;
    }
}
