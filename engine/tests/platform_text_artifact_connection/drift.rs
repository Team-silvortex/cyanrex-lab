use super::fixture::*;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_original_root_and_equal_marker_clones_are_never_adopted() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let connector = p.connector().await;
            let before = snapshot(f).await;
            let files = p.files();
            assert!(p.root.swap());
            assert_eq!(
                p.create(&connector, 0, 10, "No replacement root").await,
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
                let saved = format!("{namespace}.text_original_marker");
                f.sql(&format!(
                    "ALTER TABLE {marker} RENAME TO text_original_marker"
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
                let changed = snapshot(f).await;
                assert_eq!(
                    p.create(&connector, 0, 10, "Do not recapture original markers")
                        .await,
                    Err(unavailable())
                );
                assert_eq!(changed, snapshot(f).await);
                assert_eq!(files, p.files());
                f.sql(&format!("DROP TABLE {marker}")).await;
                f.sql(&format!("ALTER TABLE {saved} RENAME TO {table}"))
                    .await;
            }
            let created = p
                .create(
                    &connector,
                    0,
                    10,
                    "The original binding is explicitly restored",
                )
                .await
                .unwrap();
            p.counts(1).await;
            p.read_committed(&created, b"The original binding is explicitly restored")
                .await;
            p.old_files_unchanged(&files, 1);
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
