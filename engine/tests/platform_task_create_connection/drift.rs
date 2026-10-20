use super::fixture::*;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_create_original_root_and_equivalent_markers_are_never_adopted() {
    Fixture::checked(|f| Box::pin(async move {
        let p=Prepared::new(f).await;
        let connector=p.connector().await;
        let before=snapshot(f).await;
        assert!(p.root.swap());
        assert_eq!(p.create(&connector,0,task_id(10),"No replacement root").await,Err(unavailable()));
        assert!(p.root.restore());
        assert_eq!(before,snapshot(f).await);
        let marker=format!("{}.collaboration_auth_source_schema",f.source);
        f.sql(&format!("ALTER TABLE {marker} RENAME TO retained_create_source_marker")).await;
        f.sql(&format!("CREATE TABLE {marker} (LIKE {}.retained_create_source_marker INCLUDING ALL)",f.source)).await;
        f.sql(&format!("INSERT INTO {marker} SELECT * FROM {}.retained_create_source_marker",f.source)).await;
        f.sql(&format!("GRANT SELECT,UPDATE(singleton) ON {marker} TO {}",f.role)).await;
        let replaced=snapshot(f).await;
        assert_eq!(p.create(&connector,0,task_id(10),"No equivalent marker adoption").await,Err(unavailable()));
        assert_eq!(replaced,snapshot(f).await);
        f.sql(&format!("DROP TABLE {marker}")).await;
        f.sql(&format!("ALTER TABLE {}.retained_create_source_marker RENAME TO collaboration_auth_source_schema",f.source)).await;
        let created=p.create(&connector,0,task_id(10),"Original binding restored").await.unwrap();
        p.created(&created,1).await;
        drop(connector);
        assert!(super::storage_helpers::no_connections(f).await);
        assert!(p.finish());
    })).await;
}
