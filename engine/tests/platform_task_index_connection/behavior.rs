use super::fixture::*;
use cyanrex_engine::services::auth_service::durable_source::{
    DurableAuthSource, SessionCommandError, SessionTaskContentIndexWorkspace, SessionTaskError,
};
use cyanrex_engine::services::collaboration_identity_store::IdentityStoreError;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_index_private_empty_payload_pages_derive_scope_and_owner() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let first = p.task(10, 0, "Teaching plan without code").await;
            let other = p.task(20, 1, "Private collaboration notes").await;
            let last = p.task(30, 0, "Review checklist").await;
            let pooled =
                DurableAuthSource::new(f.runtime().clone(), f.config().scope().authority_id);
            let workspace =
                SessionTaskContentIndexWorkspace::new(&f.task, f.config().scope()).unwrap();
            let before = snapshot(f).await;
            assert_eq!(
                pooled
                    .list_session_content_tasks(&p.tokens[0], &workspace, None, None)
                    .await
                    .unwrap()
                    .entries,
                vec![entry(&first), entry(&last)]
            );
            assert!(before == snapshot(f).await);
            let connector = p.connector().await;
            assert_eq!(connector.origin(), f.config().origin());
            assert_eq!(connector.scope(), f.config().scope());
            let page = p.page(&connector, 0, None, Some(1)).await.unwrap();
            assert_eq!(page.entries, vec![entry(&first)]);
            assert_eq!(page.next_after, Some(first.task.reference));
            let page = p
                .page(&connector, 0, Some(first.task.reference), Some(1))
                .await
                .unwrap();
            assert_eq!(page.entries, vec![entry(&last)]);
            assert_eq!(page.next_after, None);
            let page = p
                .page(&connector, 1, Some(first.task.reference), None)
                .await
                .unwrap();
            assert_eq!(page.entries, vec![entry(&other)]);
            assert_eq!(page.next_after, None);
            assert!(p
                .page(&connector, 0, Some(last.task.reference), None)
                .await
                .unwrap()
                .entries
                .is_empty());
            drop((connector, pooled));
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_index_invalid_unbound_and_suspended_sessions_never_receive_metadata(
) {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            p.task(10, 1, "Private learner work").await;
            let connector = p.connector().await;
            assert_eq!(
                connector
                    .list_session_content_tasks("invalid", None, None)
                    .await,
                Err(invalid_session())
            );
            let denied = SessionTaskError::Session(SessionCommandError::Identity(
                IdentityStoreError::AccessDenied,
            ));
            assert_eq!(p.page(&connector, 2, None, None).await, Err(denied));
            assert_eq!(
                p.page(&connector, 1, None, None)
                    .await
                    .unwrap()
                    .entries
                    .len(),
                1
            );
            p.suspend(1).await;
            assert_eq!(p.page(&connector, 1, None, None).await, Err(denied));
            assert!(p
                .page(&connector, 0, None, None)
                .await
                .unwrap()
                .entries
                .is_empty());
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
