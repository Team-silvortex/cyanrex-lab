use super::fixture::*;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_replace_original_root_and_equivalent_marker_replacements_are_not_adopted(
) {
    Fixture::checked(|f|Box::pin(async move {
        let p=Prepared::new(f).await;let artifact=p.artifact(0,b"Keep original binding").await;
        let old=p.task_manifest(10,0,manifest("Original",vec![artifact.reference])).await;let connector=p.connector().await;
        let files=p.files();let before=snapshot(f).await;
        assert!(p.root.swap());assert_eq!(p.replace(&connector,0,&old,manifest("No new root",vec![])).await,Err(unavailable()));assert!(p.root.restore());assert_eq!(files,p.files());assert_eq!(before,snapshot(f).await);
        let marker=format!("{}.collaboration_auth_source_schema",f.source);
        f.sql(&format!("ALTER TABLE {marker} RENAME TO retained_replace_source_marker")).await;
        f.sql(&format!("CREATE TABLE {marker} (LIKE {}.retained_replace_source_marker INCLUDING ALL)",f.source)).await;
        f.sql(&format!("INSERT INTO {marker} SELECT * FROM {}.retained_replace_source_marker",f.source)).await;
        f.sql(&format!("GRANT SELECT,UPDATE(singleton) ON {marker} TO {}",f.role)).await;
        let replaced=snapshot(f).await;assert_eq!(p.replace(&connector,0,&old,manifest("No equivalent marker",vec![])).await,Err(unavailable()));assert_eq!(replaced,snapshot(f).await);
        f.sql(&format!("DROP TABLE {marker}")).await;f.sql(&format!("ALTER TABLE {}.retained_replace_source_marker RENAME TO collaboration_auth_source_schema",f.source)).await;
        let changed=p.replace(&connector,0,&old,manifest("Original restored",vec![])).await.unwrap();p.unchanged(&changed,2).await;assert_eq!(files,p.files());
        drop(connector);assert!(super::storage_helpers::no_connections(f).await);assert!(p.finish());
    })).await;
}
