use super::fixture::*;
use cyanrex_engine::models::collaboration::*;
use sqlx_core::{query::query, row::Row};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_index_checks_full_lookahead_without_reading_artifact_rows_or_bytes()
{
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let first = p.task(10, 0, "Metadata-only collaboration work").await;
            let reference = ArtifactRef {
                workspace: f.config().scope(),
                artifact_id: uuid::Uuid::from_u128(8501).try_into().unwrap(),
                revision_id: uuid::Uuid::from_u128(8502).try_into().unwrap(),
                sha256: "a".repeat(64).parse().unwrap(),
            };
            let last = p
                .task_manifest(
                    30,
                    0,
                    TaskContentManifest::new(
                        "Referenced content is not an index read",
                        vec![
                            TextPayloadBinding::new(reference, "lesson-notes.txt", "plaintext")
                                .unwrap(),
                        ],
                    )
                    .unwrap(),
                )
                .await;
            let connector = p.connector().await;
            let page = p.page(&connector, 0, None, Some(1)).await.unwrap();
            assert_eq!(page.entries, vec![entry(&first)]);
            assert_eq!(page.next_after, Some(first.task.reference));
            let page = p
                .page(&connector, 0, Some(first.task.reference), None)
                .await
                .unwrap();
            assert_eq!(page.entries, vec![entry(&last)]);
            assert_eq!(page.entries[0].payload_items, 1);
            // No Artifact rows or blob were ever created and runtime cannot SELECT Artifact business tables.
            for table in [
                "collaboration_artifacts",
                "collaboration_artifact_revisions",
            ] {
                let count: i64 = within(
                    query(&format!(
                        "SELECT count(*) AS count FROM {}.{table}",
                        f.artifacts
                    ))
                    .fetch_one(f.owner()),
                )
                .await
                .unwrap()
                .get("count");
                assert_eq!(count, 0);
            }
            let files = std::fs::read_dir(p.root.path()).unwrap().count();
            assert_eq!(
                files, 1,
                "only the independent unpublished root marker exists"
            );
            within(
                query(&format!(
                    "UPDATE {}.collaboration_task_outbox SET manifest='{{}}' WHERE task_id=$1",
                    f.task
                ))
                .bind(last.task.reference.task_id.as_uuid())
                .execute(f.owner()),
            )
            .await
            .unwrap();
            assert!(
                p.page(&connector, 0, None, Some(1)).await.is_err(),
                "corrupt owned lookahead must reject the entire page"
            );
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
