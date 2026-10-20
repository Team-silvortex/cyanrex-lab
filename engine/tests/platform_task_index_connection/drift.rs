use super::fixture::*;
use cyanrex_engine::services::auth_service::durable_source::{
    DurableAuthSource, SessionTaskContentIndexWorkspace,
};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_index_original_root_and_marker_replacements_are_not_adopted() {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await;
        let task = p.task(10, 0, "Retained original platform work").await;
        let connector = p.connector().await;
        assert_eq!(p.page(&connector, 0, None, None).await.unwrap().entries, vec![entry(&task)]);
        let before = snapshot(f).await;
        assert!(p.root.swap());
        assert_eq!(p.page(&connector, 0, None, None).await, Err(unavailable()));
        assert!(p.root.restore());
        assert!(before == snapshot(f).await);
        let marker = format!("{}.collaboration_auth_source_schema", f.source);
        f.sql(&format!("ALTER TABLE {marker} RENAME TO retained_index_source_marker")).await;
        f.sql(&format!("CREATE TABLE {marker} (LIKE {}.retained_index_source_marker INCLUDING ALL)", f.source)).await;
        f.sql(&format!("INSERT INTO {marker} SELECT * FROM {}.retained_index_source_marker", f.source)).await;
        f.sql(&format!("GRANT SELECT,UPDATE(singleton) ON {marker} TO {}", f.role)).await;
        let changed = snapshot(f).await;
        // The replacement is semantically valid. Only the retained original storage binding rejects it.
        let pooled = DurableAuthSource::new(f.runtime().clone(), f.config().scope().authority_id);
        let workspace = SessionTaskContentIndexWorkspace::new(&f.task, f.config().scope()).unwrap();
        assert_eq!(pooled.list_session_content_tasks(&p.tokens[0], &workspace, None, None).await.unwrap().entries, vec![entry(&task)]);
        assert_eq!(p.page(&connector, 0, None, None).await, Err(unavailable()));
        assert!(changed == snapshot(f).await);
        f.sql(&format!("DROP TABLE {marker}")).await;
        f.sql(&format!("ALTER TABLE {}.retained_index_source_marker RENAME TO collaboration_auth_source_schema", f.source)).await;
        assert!(before == snapshot(f).await);
        assert_eq!(p.page(&connector, 0, None, None).await.unwrap().entries, vec![entry(&task)]);
        drop((pooled, connector));
        assert!(super::storage_helpers::no_connections(f).await);
        assert!(p.finish());
    })).await;
}
