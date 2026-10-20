use super::fixture::*;
use cyanrex_engine::models::collaboration::*;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_replace_empty_draft_title_commits_one_revision_event_without_file_changes(
) {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let old = p.task(10, 0, "协作任务可以没有任何 payload").await;
            let connector = p.connector().await;
            assert_eq!(connector.origin(), f.config().origin());
            assert_eq!(connector.scope(), f.config().scope());
            let files = p.files();
            let immutable = p.immutable().await;
            let changed = p
                .replace(
                    &connector,
                    0,
                    &old,
                    manifest("新的备课标题😀，不要求代码或运行", vec![]),
                )
                .await
                .unwrap();
            assert_eq!(changed.task.reference, old.task.reference);
            assert_eq!(changed.task.owner, old.task.owner);
            assert_eq!(changed.task.created_at, old.task.created_at);
            assert_eq!(changed.task.status, TaskStatus::Draft);
            assert_eq!(u64::from(changed.task.revision), 2);
            assert!(changed.manifest.payload().is_empty());
            assert_eq!(changed.task.title, "新的备课标题😀，不要求代码或运行");
            assert!(changed.task.definition.is_none());
            p.unchanged(&changed, 2).await;
            p.reopened(&changed).await;
            assert_eq!(p.count("collaboration_tasks").await, 1);
            assert_eq!(files, p.files());
            assert_eq!(immutable, p.immutable().await);
            super::ordered::run(&p, &connector, &changed).await;
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
