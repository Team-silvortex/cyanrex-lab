use super::fixture::*;
use cyanrex_engine::{
    models::collaboration::*,
    services::{auth_service::durable_source::SessionTaskError, task_store::TaskStoreError},
};
use sqlx_core::query::query;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_replace_revision_lifecycle_and_current_private_identity_never_widen_access(
) {
    Fixture::checked(|f|Box::pin(async move {
        let p=Prepared::new(f).await;let old=p.task(10,0,"Private task").await;let connector=p.connector().await;
        let first=p.replace(&connector,0,&old,manifest("Committed edit",vec![])).await.unwrap();
        let before=snapshot(f).await;let files=p.files();
        assert_eq!(p.replace(&connector,0,&old,manifest("Stale edit",vec![])).await,Err(SessionTaskError::Task(TaskStoreError::StaleRevision)));
        assert_eq!(p.replace(&connector,0,&first,first.manifest.clone()).await,Err(SessionTaskError::Task(TaskStoreError::InvalidInput)));
        assert_eq!(p.replace(&connector,1,&first,manifest("No teacher shortcut",vec![])).await,Err(SessionTaskError::Task(TaskStoreError::NotFound)));
        assert!(p.replace(&connector,2,&first,manifest("No unaudited actor",vec![])).await.is_err());
        assert_eq!(connector.replace_session_content_task("invalid",task_id(10),first.task.revision,manifest("Invalid",vec![])).await,Err(invalid_session()));
        assert_eq!(connector.replace_session_content_task(&p.tokens[0],task_id(999),first.task.revision,manifest("Missing",vec![])).await,Err(SessionTaskError::Task(TaskStoreError::NotFound)));
        assert_eq!(before,snapshot(f).await);assert_eq!(files,p.files());
        let (source,workspace)=p.pooled();
        let ready=source.transition_session_content_task(&p.tokens[0],&workspace,first.task.reference.task_id,first.task.revision,TaskStatus::Ready).await.unwrap();
        assert_eq!(p.replace(&connector,0,&ready,manifest("No non-Draft edit",vec![])).await,Err(SessionTaskError::Task(TaskStoreError::InvalidTransition)));
        p.unchanged(&ready,3).await;
        let current=p.task(30,0,"Still-editable Task for authority guards").await;
        let current=p.replace(&connector,0,&current,manifest("Active owner can edit this Draft",vec![])).await.unwrap();
        within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token=$1",f.source)).bind(token_hash(&p.tokens[0])).execute(f.owner())).await.unwrap();
        let expired=snapshot(f).await;
        assert_eq!(p.replace(&connector,0,&current,manifest("No expired owner",vec![])).await,Err(invalid_session()));assert_eq!(expired,snapshot(f).await);
        within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+interval '12 hours' WHERE token=$1",f.source)).bind(token_hash(&p.tokens[0])).execute(f.owner())).await.unwrap();
        p.suspend(0).await;let suspended=snapshot(f).await;
        assert!(p.replace(&connector,0,&current,manifest("No suspended owner",vec![])).await.is_err());assert_eq!(suspended,snapshot(f).await);
        let owned=p.task(20,1,"Task owned by the retiring account").await;
        let owned=p.replace(&connector,1,&owned,manifest("Owner one can edit before retirement",vec![])).await.unwrap();
        f.sql(&format!("DELETE FROM {}.users WHERE username='index-owner-1'",f.source)).await;
        let replacement=uuid::Uuid::new_v4();
        within(query(&format!("INSERT INTO {}.users(username,password_salt,password_hash,totp_secret,account_id) VALUES('index-owner-1','synthetic','noncredential','nonenrollment',$1)",f.source)).bind(replacement).execute(f.owner())).await.unwrap();
        within(query(&format!("INSERT INTO {}.sessions(token,username,account_id,expires_at) VALUES($1,'index-owner-1',$2,clock_timestamp()+interval '12 hours')",f.source)).bind(token_hash(&p.tokens[1])).bind(replacement).execute(f.owner())).await.unwrap();
        let recreated=snapshot(f).await;assert!(p.replace(&connector,1,&owned,manifest("No reincarnation inheritance",vec![])).await.is_err());assert_eq!(recreated,snapshot(f).await);
        assert_eq!(files,p.files());drop((connector,source,workspace));assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
    })).await;
}
