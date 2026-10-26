use super::fixture::*;
use cyanrex_engine::models::collaboration::MAX_TEXT_PAYLOAD_BYTES;
use cyanrex_engine::services::{
    artifact_store::ArtifactStoreError, auth_service::durable_source::SessionArtifactError,
};
use std::{fs, os::unix::fs::MetadataExt};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_commits_exact_empty_unicode_and_maximum_text() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let connector = p.connector().await;
            assert_eq!(connector.origin(), f.config().origin());
            assert_eq!(connector.scope(), f.config().scope());
            let original = p.files();
            let unchanged = p.non_artifacts().await;
            for (index, text) in [
                String::new(),
                "协作讲义😀\r\n正文不是程序\t한글\n".to_owned(),
                "x".repeat(MAX_TEXT_PAYLOAD_BYTES),
            ]
            .into_iter()
            .enumerate()
            {
                let n = index as u128 + 1;
                let created = p.create(&connector, 0, n, &text).await.unwrap();
                assert_eq!(created.reference, p.reference(n, text.as_bytes()));
                assert_eq!(created.owner, p.owners[0]);
                assert_eq!(u64::from(created.sequence), 1);
                assert!(created.parent.is_none());
                assert_eq!(created.kind.as_str(), "cyanrex.task-text");
                assert_eq!(created.media_type, "text/plain");
                assert_eq!(created.byte_length, text.len() as u64);
                assert_eq!(created.title, "协作讲义，不要求代码或执行");
                let metadata = fs::symlink_metadata(p.blob(&created.reference)).unwrap();
                assert_eq!(metadata.mode() & 0o7777, 0o400);
                assert_eq!(metadata.nlink(), 1);
                p.read_committed(&created, text.as_bytes()).await;
                p.counts(index as i64 + 1).await;
                p.old_files_unchanged(&original, index + 1);
                assert_eq!(unchanged, p.non_artifacts().await);
            }
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_invalid_text_title_and_token_never_dispatch() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let connector = p.connector().await;
            let before = snapshot(f).await;
            let files = p.files();
            assert!(p.root.swap());
            // An invalid request must fail before even checking this deliberately replaced root.
            for (title, text) in [
                (String::new(), "valid".to_owned()),
                (" \t".into(), "valid".into()),
                ("x".repeat(257), "valid".into()),
                ("😀".repeat(65), "valid".into()),
                ("Bad\nTitle".into(), "valid".into()),
                ("Valid".into(), "x".repeat(MAX_TEXT_PAYLOAD_BYTES + 1)),
                ("Valid".into(), "NUL\0".into()),
                ("Valid".into(), "DEL\u{7f}".into()),
                ("Valid".into(), "ESC\u{1b}".into()),
            ] {
                assert_eq!(
                    connector
                        .create_session_text_artifact(
                            &p.tokens[0],
                            artifact_id(10),
                            revision_id(10),
                            title,
                            text
                        )
                        .await,
                    Err(SessionArtifactError::Artifact(
                        ArtifactStoreError::InvalidInput
                    ))
                );
            }
            assert_eq!(
                connector
                    .create_session_text_artifact(
                        "invalid",
                        artifact_id(10),
                        revision_id(10),
                        "Valid".into(),
                        "Valid".into()
                    )
                    .await,
                Err(invalid_session())
            );
            assert!(p.root.restore());
            assert_eq!(before, snapshot(f).await);
            assert_eq!(files, p.files());
            p.counts(0).await;
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
