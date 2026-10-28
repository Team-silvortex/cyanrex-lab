use super::fixture::*;
use axum::http::StatusCode;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_http_original_storage_grants_and_readonly_fail_closed(
) {
    for mode in [
        "root",
        "source-marker",
        "task-marker",
        "artifact-marker",
        "task-lock-grant",
        "artifact-event-grant",
        "readonly",
    ] {
        Fixture::checked(|f| {
            Box::pin(async move {
                let p = Prepared::new(f).await;
                let old = p.task(10, 0, "Original private binding").await;
                let route = router(&p, 1).await;
                let original_files = p.files();
                match mode {
                    "root" => assert!(p.root.swap()),
                    "task-lock-grant" => {
                        f.sql(&format!(
                            "REVOKE UPDATE(task_id) ON {}.collaboration_tasks FROM {}",
                            f.task, f.role
                        ))
                        .await
                    }
                    "artifact-event-grant" => {
                        f.sql(&format!(
                            "REVOKE INSERT ON {}.collaboration_artifact_outbox FROM {}",
                            f.artifacts, f.role
                        ))
                        .await
                    }
                    "readonly" => {
                        f.sql(&format!(
                            "ALTER ROLE {} IN DATABASE {} SET default_transaction_read_only=on",
                            f.role, f.database
                        ))
                        .await
                    }
                    _ => {
                        let (namespace, table) = match mode {
                            "source-marker" => (&f.source, "collaboration_auth_source_schema"),
                            "task-marker" => (&f.task, "collaboration_task_schema"),
                            _ => (&f.artifacts, "collaboration_artifact_schema"),
                        };
                        let marker = format!("{namespace}.{table}");
                        let saved = format!("{namespace}.task_text_http_original_marker");
                        f.sql(&format!(
                            "ALTER TABLE {marker} RENAME TO task_text_http_original_marker"
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
                    }
                }
                let before = snapshot(f).await;
                let acl = p.acl().await;
                error(
                    &route,
                    request(
                        &p,
                        0,
                        "POST",
                        &path(&old),
                        wire(&old, 1, "No hidden storage repair"),
                    ),
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service_unavailable",
                    "unconfirmed",
                )
                .await;
                assert_eq!(before, snapshot(f).await);
                assert_eq!(acl, p.acl().await);
                p.counts(0).await;
                if mode == "root" {
                    assert!(p.root.restore());
                }
                if mode == "artifact-event-grant" {
                    p.old_files_unchanged(&original_files, 1);
                    assert_eq!(
                        std::fs::read(p.blob(&p.reference(1, b"No hidden storage repair")))
                            .unwrap(),
                        b"No hidden storage repair"
                    );
                } else {
                    assert_eq!(original_files, p.files());
                }
                drop(route);
                assert!(super::storage_helpers::no_connections(f).await);
                assert!(p.finish());
            })
        })
        .await;
    }
}
