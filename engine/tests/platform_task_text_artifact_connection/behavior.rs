use super::fixture::*;
use cyanrex_engine::models::collaboration::MAX_TEXT_PAYLOAD_BYTES;
use std::{fs, os::unix::fs::MetadataExt};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_commits_private_bytes_without_mutating_task_or_source(
) {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            // A structurally valid old pin need not have materialized bytes. This command guards
            // exact Task metadata only; it neither reads nor claims to verify the old payload.
            let old = p
                .task_manifest(
                    10,
                    1,
                    manifest(
                        "普通协作者的协作讲义，不要求代码",
                        vec![p.reference(777, b"Unmaterialized old pin")],
                    ),
                )
                .await;
            let connector = p.connector().await;
            assert_eq!(connector.origin(), f.config().origin());
            assert_eq!(connector.scope(), f.config().scope());
            let before = p.non_artifacts().await;
            let catalog = f.snapshot().await;
            let files = p.files();
            for (index, text) in [
                String::new(),
                "새 협업 노트😀\nShared seminar notes, not executable code".to_owned(),
                "x".repeat(MAX_TEXT_PAYLOAD_BYTES),
            ]
            .into_iter()
            .enumerate()
            {
                let created = p
                    .create(&connector, 1, &old, index as u128 + 1, &text)
                    .await
                    .unwrap();
                assert_eq!(created.owner, p.owners[1]);
                assert_eq!(created.title, old.task.title);
                assert_eq!(
                    created.reference,
                    p.reference(index as u128 + 1, text.as_bytes())
                );
                assert_eq!(created.byte_length, text.len() as u64);
                assert_eq!(created.kind.as_str(), "cyanrex.task-text");
                assert_eq!(created.media_type, "text/plain");
                assert_eq!(u64::from(created.sequence), 1);
                assert_eq!(created.parent, None);
                p.read_committed(&created, text.as_bytes()).await;
                let metadata = fs::symlink_metadata(p.blob(&created.reference)).unwrap();
                assert_eq!(metadata.mode() & 0o777, 0o400);
                assert_eq!(metadata.nlink(), 1);
                assert_eq!(before, p.non_artifacts().await);
                p.unchanged(&old).await;
            }
            assert!(catalog == f.snapshot().await);
            p.counts(3).await;
            p.old_files_unchanged(&files, 3);
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
