use super::support::*;
use crate::{fixture::*, *};
#[tokio::test]
#[ignore = "requires explicit disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_sharing_compares_all_published_coordinates_and_exact_owner_generations() {
    checked(Fixture::ready().await,|f|Box::pin(async move{
        for(field,value)in[("file_id",serde_json::json!(Uuid::new_v4())),("version_id",serde_json::json!(Uuid::new_v4())),("byte_length",serde_json::json!(18)),("sha256",serde_json::json!("b".repeat(64)))]{let mut c=f.command(1);let mut raw=serde_json::to_value(&c.expected_file).unwrap();raw[field]=value;c.expected_file=serde_json::from_value(raw).unwrap();assert_eq!(issue_as(f,&f.base.grants.auth.learner_token,&f.workspace,&c).await,Err(SessionFileSharingError::PermissionDenied),"isolated exact file coordinate {field}");}
        for field in["authority_id","workspace_id"]{let mut c=f.command(1);let mut raw=serde_json::to_value(&c.expected_file).unwrap();raw["workspace"][field]=serde_json::json!(Uuid::new_v4());c.expected_file=serde_json::from_value(raw).unwrap();assert_eq!(issue_as(f,&f.base.grants.auth.learner_token,&f.workspace,&c).await,Err(recipient(FileRecipientGrantStoreError::ScopeMismatch)));}
        let mut missing=f.command(1);missing.upload_id=publication_fixture::upload(99);assert_eq!(issue_as(f,&f.base.grants.auth.learner_token,&f.workspace,&missing).await,Err(SessionFileSharingError::PermissionDenied));
        let mut c=f.command(1);c.recipient=FileAccountRef::new(f.recipient.principal(),id());assert_eq!(issue_as(f,&f.base.grants.auth.learner_token,&f.workspace,&c).await,Err(session(SessionCommandError::AccountMismatch)));
        let mut c=f.command(1);c.recipient=FileAccountRef::new(PrincipalRef{principal_id:id(),..f.recipient.principal()},f.recipient.account_id());assert_eq!(issue_as(f,&f.base.grants.auth.learner_token,&f.workspace,&c).await,Err(identity(IdentityStoreError::StaleIdentity)));
        f.base.grants.issue(2,f.base.grants.issuer()).await;let mut wrong=f.command(1);wrong.publisher_grant_id=grant_fixture::grant(2);assert_eq!(issue_as(f,&f.base.grants.auth.manager_token,&f.workspace,&wrong).await,Err(SessionFileSharingError::PermissionDenied),"a teacher/deployment manager's own grant is not ownership of another Published row");
        let old=issue(f,1).await.unwrap();assert_eq!(old.grant().file(),&f.expected_file);
        // A valid canonical publication with only its owner-account scalar changed is no
        // longer this actor's record. The old account must not inherit the new generation.
        let new_id=Uuid::new_v4();let mut holder=f.base.grants.auth.base.admin.begin().await.unwrap();query(&format!("UPDATE {}.{PUB_ROWS} SET account_id=$1 WHERE upload_id=$2",f.base.publication_schema)).bind(new_id).bind(publication_fixture::upload(1).as_uuid()).execute(&mut *holder).await.unwrap();let injected=snapshot_in_tx(f,&mut holder).await;holder.commit().await.unwrap();assert_eq!(issue(f,2).await,Err(SessionFileSharingError::PermissionDenied));assert_eq!(f.snapshot().await,injected);
        query(&format!("UPDATE {}.{PUB_ROWS} SET account_id=$1 WHERE upload_id=$2",f.base.publication_schema)).bind(f.actor().account_id().as_uuid()).bind(publication_fixture::upload(1).as_uuid()).execute(&f.base.grants.auth.base.admin).await.unwrap();issue(f,2).await.unwrap();
    })).await;
}
