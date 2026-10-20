use super::fixture::*;
use cyanrex_engine::{
    models::collaboration::*,
    platform_host::PlatformTaskIndexConnector,
    services::{auth_service::durable_source::SessionTaskError, task_store::TaskStoreError},
};
use sqlx_core::query::query;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_create_commits_empty_draft_and_owner_private_list_reopen() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let connector = p.connector().await;
            assert_eq!(connector.origin(), f.config().origin());
            assert_eq!(connector.scope(), f.config().scope());
            let title = "协作教学提纲😀：任务可以没有代码、执行或任何 payload";
            let created = p.create(&connector, 0, task_id(10), title).await.unwrap();
            assert_eq!(created.task.owner, p.owners[0]);
            assert_eq!(created.task.reference.workspace, f.config().scope());
            assert_eq!(created.task.reference.task_id, task_id(10));
            assert_eq!(created.task.title, title);
            assert_eq!(created.manifest.title(), title);
            assert_eq!(created.task.status, TaskStatus::Draft);
            assert_eq!(u64::from(created.task.revision), 1);
            assert!(created.task.definition.is_none() && created.task.input_refs.is_empty());
            assert!(created.manifest.payload().is_empty());
            p.created(&created, 1).await;
            p.enable_reopen().await;
            let reader =
                cyanrex_engine::platform_host::PlatformTaskContentConnector::new(p.storage().await);
            let reopened = reader
                .get_session_content_task(&p.tokens[0], created.task.reference.task_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(reopened.snapshot, created);
            assert!(reopened.contents.is_empty());
            let index = PlatformTaskIndexConnector::new(p.storage().await);
            let page = index
                .list_session_content_tasks(&p.tokens[0], None, None)
                .await
                .unwrap();
            assert_eq!(page.entries.len(), 1);
            assert_eq!(page.entries[0].reference, created.task.reference);
            assert_eq!(page.entries[0].payload_items, 0);
            assert!(index
                .list_session_content_tasks(&p.tokens[1], None, None)
                .await
                .unwrap()
                .entries
                .is_empty());
            drop((connector, index, reader));
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_create_duplicates_and_current_identity_never_adopt_an_owner() {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await;
        let connector = p.connector().await;
        let created = p.create(&connector, 0, task_id(10), "Original private task").await.unwrap();
        for owner in [0,1] {
            assert_eq!(p.create(&connector, owner, task_id(10), "Cannot overwrite or adopt").await,
                Err(SessionTaskError::Task(TaskStoreError::Conflict)));
        }
        let before = snapshot(f).await;
        assert_eq!(connector.create_session_empty_content_task("invalid",task_id(20),"No invalid Session").await, Err(invalid_session()));
        assert!(p.create(&connector,2,task_id(20),"No unaudited owner").await.is_err());
        assert_eq!(before,snapshot(f).await);
        within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token=$1",f.source))
            .bind(token_hash(&p.tokens[1])).execute(f.owner())).await.unwrap();
        let expired = snapshot(f).await;
        assert_eq!(p.create(&connector,1,task_id(20),"No expired Session").await,Err(invalid_session()));
        assert_eq!(expired,snapshot(f).await);
        within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+interval '12 hours' WHERE token=$1",f.source))
            .bind(token_hash(&p.tokens[1])).execute(f.owner())).await.unwrap();
        p.suspend(1).await;
        let suspended = snapshot(f).await;
        assert!(p.create(&connector,1,task_id(20),"No suspended member").await.is_err());
        assert_eq!(suspended,snapshot(f).await);
        let (a,b) = tokio::join!(p.create(&connector,0,task_id(30),"Race A"),p.create(&connector,0,task_id(30),"Race B"));
        assert!(matches!((&a,&b),(Ok(_),Err(SessionTaskError::Task(TaskStoreError::Conflict)))|(Err(SessionTaskError::Task(TaskStoreError::Conflict)),Ok(_))));
        p.created(&created,2).await;
        f.sql(&format!("DELETE FROM {}.users WHERE username='index-owner-0'",f.source)).await;
        let replacement=uuid::Uuid::new_v4();
        within(query(&format!("INSERT INTO {}.users(username,password_salt,password_hash,totp_secret,account_id) VALUES('index-owner-0','synthetic','noncredential','nonenrollment',$1)",f.source))
            .bind(replacement).execute(f.owner())).await.unwrap();
        within(query(&format!("INSERT INTO {}.sessions(token,username,account_id,expires_at) VALUES($1,'index-owner-0',$2,clock_timestamp()+interval '12 hours')",f.source))
            .bind(token_hash(&p.tokens[0])).bind(replacement).execute(f.owner())).await.unwrap();
        let recreated=snapshot(f).await;
        assert!(p.create(&connector,0,task_id(40),"A recreated username cannot inherit the audited owner").await.is_err());
        assert_eq!(recreated,snapshot(f).await);
        drop(connector);
        assert!(super::storage_helpers::no_connections(f).await);
        assert!(p.finish());
    })).await;
}
