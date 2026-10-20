use super::fixture::*;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_content_original_root_and_equivalent_marker_replacements_are_not_adopted(
) {
    Fixture::checked(|f| Box::pin(async move {
        let p = Prepared::new(f).await;
        let artifact = p.artifact(0, "保留原始存储绑定".as_bytes()).await;
        let task = p.task_manifest(10, 0, manifest("原目标", vec![artifact.reference])).await;
        let connector = p.connector().await;
        let original = p.read(&connector, 0, &task).await.unwrap();
        assert!(original.is_some());
        assert!(p.root.swap());
        assert_eq!(p.read(&connector, 0, &task).await, Err(unavailable()));
        assert!(p.root.restore());
        let (pooled, workspace) = p.pooled();
        let marker = format!("{}.collaboration_auth_source_schema", f.source);
        f.sql(&format!("ALTER TABLE {marker} RENAME TO retained_content_source_marker")).await;
        f.sql(&format!("CREATE TABLE {marker} (LIKE {}.retained_content_source_marker INCLUDING ALL)", f.source)).await;
        f.sql(&format!("INSERT INTO {marker} SELECT * FROM {}.retained_content_source_marker", f.source)).await;
        f.sql(&format!("GRANT SELECT,UPDATE(singleton) ON {marker} TO {}", f.role)).await;
        assert_eq!(pooled.get_session_content_task(&p.tokens[0], &workspace, task.task.reference.task_id).await.unwrap(), original,
            "replacement is semantically valid; retained identity is the rejecting boundary");
        assert_eq!(p.read(&connector, 0, &task).await, Err(unavailable()));
        f.sql(&format!("DROP TABLE {marker}")).await;
        f.sql(&format!("ALTER TABLE {}.retained_content_source_marker RENAME TO collaboration_auth_source_schema", f.source)).await;
        assert_eq!(p.read(&connector, 0, &task).await.unwrap(), original);
        drop((connector, pooled, workspace));
        assert!(super::storage_helpers::no_connections(f).await);
        assert!(p.finish());
    })).await;
}
