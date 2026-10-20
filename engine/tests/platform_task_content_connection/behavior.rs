use super::fixture::*;
use sqlx_core::query::query;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_content_reopens_empty_and_ordered_exact_unicode_revisions() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let empty = p.task(10, 0, "备课提纲，不需要代码或运行记录").await;
            let old = p.artifact(0, "\t中文教学笔记😀\r\n".as_bytes()).await;
            let new = p.revise(&old, "新的讲义版本".as_bytes()).await;
            let task = p
                .task_manifest(
                    20,
                    0,
                    manifest(
                        "按原顺序重开协作内容",
                        vec![new.reference.clone(), old.reference.clone()],
                    ),
                )
                .await;
            let (source, workspace) = p.pooled();
            let expected = source
                .get_session_content_task(&p.tokens[0], &workspace, task.task.reference.task_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(expected.snapshot, task);
            assert_eq!(expected.contents[0].bytes, "新的讲义版本".as_bytes());
            assert_eq!(
                expected.contents[1].bytes,
                "\t中文教学笔记😀\r\n".as_bytes()
            );
            let connector = p.connector().await;
            assert_eq!(connector.origin(), f.config().origin());
            assert_eq!(connector.scope(), f.config().scope());
            let reopened = p.read(&connector, 0, &empty).await.unwrap().unwrap();
            assert_eq!(reopened.snapshot, empty);
            assert!(reopened.contents.is_empty());
            assert_eq!(p.read(&connector, 0, &task).await.unwrap(), Some(expected));
            drop((connector, source, workspace));
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_content_current_identity_and_private_owner_never_inherit_access() {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await;
        let artifact = p.artifact(1, "学生的私人讲义".as_bytes()).await;
        let task = p.task_manifest(10, 1, manifest("不能凭教师角色读取", vec![artifact.reference])).await;
        let connector = p.connector().await;
        assert!(p.read(&connector, 1, &task).await.unwrap().is_some());
        assert_eq!(p.read(&connector, 0, &task).await.unwrap(), None);
        assert_eq!(connector.get_session_content_task(&p.tokens[0], task_id(999)).await.unwrap(), None);
        assert_eq!(connector.get_session_content_task("invalid", task.task.reference.task_id).await, Err(invalid_session()));
        assert!(p.read(&connector, 2, &task).await.is_err(), "unbound authenticated account has no private owner");
        within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token=$1", f.source))
            .bind(token_hash(&p.tokens[1])).execute(f.owner())).await.unwrap();
        assert_eq!(p.read(&connector, 1, &task).await, Err(invalid_session()));
        within(query(&format!("UPDATE {}.sessions SET expires_at=clock_timestamp()+interval '12 hours' WHERE token=$1", f.source))
            .bind(token_hash(&p.tokens[1])).execute(f.owner())).await.unwrap();
        p.suspend(1).await;
        assert!(p.read(&connector, 1, &task).await.is_err());
        f.sql(&format!("DELETE FROM {}.users WHERE username='index-owner-1'", f.source)).await;
        assert_eq!(p.read(&connector, 1, &task).await, Err(invalid_session()));
        let replacement = uuid::Uuid::new_v4();
        within(query(&format!("INSERT INTO {}.users(username,password_salt,password_hash,totp_secret,account_id) VALUES('index-owner-1','synthetic','noncredential','nonenrollment',$1)", f.source))
            .bind(replacement).execute(f.owner())).await.unwrap();
        within(query(&format!("INSERT INTO {}.sessions(token,username,account_id,expires_at) VALUES($1,'index-owner-1',$2,clock_timestamp()+interval '12 hours')", f.source))
            .bind(token_hash(&p.tokens[1])).bind(replacement).execute(f.owner())).await.unwrap();
        assert!(p.read(&connector, 1, &task).await.is_err(), "same username/token cannot inherit the retired audited account incarnation");
        drop(connector);
        assert!(super::storage_helpers::no_connections(f).await);
        assert!(p.finish());
    })).await;
}
