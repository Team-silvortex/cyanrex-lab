use super::fixture::*;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_original_root_and_marker_identity_are_not_recaptured()
{
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let old = p.task(10, 0, "Original private binding").await;
            let connector = p.connector().await;
            let before = snapshot(f).await;
            let files = p.files();
            assert!(p.root.swap());
            assert_eq!(
                p.create(&connector, 0, &old, 1, "No replacement root")
                    .await,
                Err(unavailable())
            );
            assert!(p.root.restore());
            assert_eq!(before, snapshot(f).await);
            assert_eq!(files, p.files());
            for (namespace, table) in [
                (&f.source, "collaboration_auth_source_schema"),
                (&f.task, "collaboration_task_schema"),
                (&f.artifacts, "collaboration_artifact_schema"),
            ] {
                let marker = format!("{namespace}.{table}");
                let saved = format!("{namespace}.task_text_saved_marker");
                f.sql(&format!(
                    "ALTER TABLE {marker} RENAME TO task_text_saved_marker"
                ))
                .await;
                f.sql(&format!(
                    "CREATE TABLE {marker} (LIKE {saved} INCLUDING ALL)"
                ))
                .await;
                f.sql(&format!("INSERT INTO {marker} SELECT * FROM {saved}"))
                    .await;
                f.sql(&format!(
                    "GRANT SELECT,UPDATE(singleton) ON {marker} TO {}",
                    f.role
                ))
                .await;
                p.reject(&connector, 0, &old, 1, unavailable()).await;
                f.sql(&format!("DROP TABLE {marker}")).await;
                f.sql(&format!("ALTER TABLE {saved} RENAME TO {table}"))
                    .await;
            }
            let created = p
                .create(&connector, 0, &old, 1, "Only restored original binding")
                .await
                .unwrap();
            p.read_committed(&created, b"Only restored original binding")
                .await;
            p.counts(1).await;
            p.unchanged(&old).await;
            p.old_files_unchanged(&files, 1);
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
