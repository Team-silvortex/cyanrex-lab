use super::fixture::*;
use axum::http::StatusCode;

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_publication_original_binding_owner_revision_and_grants() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let old = p.task(10, 0, "Original Task ownership").await;
            let route = router(&p, [1; 8]).await;
            let files = p.files();
            let before = snapshot(f).await;
            assert!(p.root.swap());
            error(
                &route,
                request(
                    &p,
                    0,
                    "POST",
                    &path(&old),
                    wire(&old, 1, "Do not recapture root"),
                ),
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                "unconfirmed",
            )
            .await;
            assert!(p.root.restore());
            assert_eq!(before, snapshot(f).await);
            assert_eq!(files, p.files());
            for (namespace, table) in [
                (&f.source, "collaboration_auth_source_schema"),
                (&f.task, "collaboration_task_schema"),
                (&f.artifacts, "collaboration_artifact_schema"),
            ] {
                let marker = format!("{namespace}.{table}");
                let saved = format!("{namespace}.task_host_saved_marker");
                f.sql(&format!(
                    "ALTER TABLE {marker} RENAME TO task_host_saved_marker"
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
                error(
                    &route,
                    request(
                        &p,
                        0,
                        "POST",
                        &path(&old),
                        wire(&old, 1, "No equal marker adoption"),
                    ),
                    StatusCode::SERVICE_UNAVAILABLE,
                    "service_unavailable",
                    "unconfirmed",
                )
                .await;
                assert_eq!(changed, snapshot(f).await);
                assert_eq!(files, p.files());
                f.sql(&format!("DROP TABLE {marker}")).await;
                f.sql(&format!("ALTER TABLE {saved} RENAME TO {table}"))
                    .await;
            }
            for table in [
                "collaboration_artifacts",
                "collaboration_artifact_revisions",
                "collaboration_artifact_outbox",
            ] {
                f.sql(&format!(
                    "REVOKE INSERT ON {}.{table} FROM {}",
                    f.artifacts, f.role
                ))
                .await;
            }
            let before = snapshot(f).await;
            error(
                &route,
                request(
                    &p,
                    1,
                    "POST",
                    &path(&old),
                    wire(&old, 1, "No cross-owner write"),
                ),
                StatusCode::NOT_FOUND,
                "not_found",
                "unconfirmed",
            )
            .await;
            let mut stale = old.clone();
            stale.task.revision = 2.try_into().unwrap();
            error(
                &route,
                request(
                    &p,
                    0,
                    "POST",
                    &path(&old),
                    wire(&stale, 1, "No stale write"),
                ),
                StatusCode::CONFLICT,
                "stale_revision",
                "unconfirmed",
            )
            .await;
            assert_eq!(before, snapshot(f).await);
            assert_eq!(files, p.files());
            for table in [
                "collaboration_artifacts",
                "collaboration_artifact_revisions",
                "collaboration_artifact_outbox",
            ] {
                f.sql(&format!(
                    "GRANT INSERT ON {}.{table} TO {}",
                    f.artifacts, f.role
                ))
                .await;
            }
            // The new lane may not inherit a business-write dependency from the older read/replace lanes.
            f.sql(&format!(
                "REVOKE UPDATE(revision,snapshot,manifest) ON {}.collaboration_tasks FROM {}",
                f.task, f.role
            ))
            .await;
            f.sql(&format!(
                "GRANT UPDATE(task_id) ON {}.collaboration_tasks TO {}",
                f.task, f.role
            ))
            .await;
            f.sql(&format!(
                "REVOKE UPDATE(artifact_id) ON {}.collaboration_artifacts FROM {}",
                f.artifacts, f.role
            ))
            .await;
            let granted = snapshot(f).await;
            call(
                &route,
                request(
                    &p,
                    0,
                    "POST",
                    &path(&old),
                    wire(&old, 1, "Narrow Task guard only"),
                ),
                StatusCode::CREATED,
            )
            .await;
            assert_eq!(
                p.non_artifacts().await,
                granted
                    .into_iter()
                    .filter(|(table, _)| !table.starts_with(&format!("{}.", f.artifacts)))
                    .collect::<Vec<_>>()
            );
            f.sql(&format!(
                "REVOKE UPDATE(task_id) ON {}.collaboration_tasks FROM {}",
                f.task, f.role
            ))
            .await;
            let before = snapshot(f).await;
            let new_files = p.files();
            error(
                &route,
                request(
                    &p,
                    0,
                    "POST",
                    &path(&old),
                    wire(&old, 2, "No automatic rowlock grant"),
                ),
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                "unconfirmed",
            )
            .await;
            assert_eq!(before, snapshot(f).await);
            assert_eq!(new_files, p.files());
            p.counts(1).await;
            unchanged(&p, 10, 0, &old).await;
            drop(route);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
