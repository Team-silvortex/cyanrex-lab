use super::fixture::*;
use axum::http::StatusCode;
use cyanrex_engine::models::collaboration::{ArtifactRevision, MAX_TEXT_PAYLOAD_BYTES};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_text_artifact_http_private_roundtrip_and_separate_task_reference_bridge()
{
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let route = router(&p, 4).await;
            let files = p.files();
            let private = p.non_artifacts().await;
            let texts = [
                String::new(),
                "协作讲义😀\r\n不是代码\t한국어\n".to_owned(),
                "x".repeat(MAX_TEXT_PAYLOAD_BYTES),
            ];
            let mut references = Vec::new();
            for (index, text) in texts.iter().enumerate() {
                let n = index as u128 + 1;
                let body = call(
                    &route,
                    request(&p, 0, "POST", PATH, wire(n, "私人讨论笔记", &text)),
                    StatusCode::CREATED,
                )
                .await;
                assert_eq!(body.as_object().unwrap().len(), 2);
                assert_eq!(body["schema_version"], 1);
                assert!(body.get("text").is_none());
                let artifact: ArtifactRevision =
                    serde_json::from_value(body["artifact"].clone()).unwrap();
                assert_eq!(artifact.owner, p.owners[0]);
                assert_eq!(artifact.reference, p.reference(n, text.as_bytes()));
                assert_eq!(artifact.kind.as_str(), "cyanrex.task-text");
                assert_eq!(artifact.media_type, "text/plain");
                assert_eq!(artifact.byte_length, text.len() as u64);
                assert_eq!(u64::from(artifact.sequence), 1);
                assert!(artifact.parent.is_none());
                p.read_committed(&artifact, text.as_bytes()).await;
                p.counts(index as i64 + 1).await;
                p.old_files_unchanged(&files, index + 1);
                assert_eq!(private, p.non_artifacts().await);
                references.push(artifact.reference);
            }
            // Publication itself creates no Task or manifest. The already-existing separate
            // command is the sole bridge; its trusted pool is not a grant to the HTTP role.
            let (source, workspace) = p.pooled();
            let id = super::content_fixture::task_id(41);
            let manifest =
                super::content_fixture::manifest("Separate teaching task", references.clone());
            let task = source
                .create_session_content_task(&p.tokens[0], &workspace, id, manifest.clone())
                .await
                .unwrap();
            let reopened = source
                .get_session_content_task(&p.tokens[0], &workspace, id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(task, reopened.snapshot);
            assert_eq!(reopened.snapshot.manifest, manifest);
            assert_eq!(reopened.contents.len(), texts.len());
            for ((content, reference), text) in
                reopened.contents.iter().zip(&references).zip(&texts)
            {
                assert_eq!(&content.revision.reference, reference);
                assert_eq!(&content.bytes, text.as_bytes());
            }
            assert!(source
                .get_session_content_task(&p.tokens[1], &workspace, id)
                .await
                .unwrap()
                .is_none());
            for path in ["/platform/v1/tasks", "/platform/v1/text-artifacts/extra"] {
                error(
                    &route,
                    request(&p, 0, "POST", path, unread()),
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "not_attempted",
                )
                .await;
            }
            p.counts(3).await;
            p.old_files_unchanged(&files, 3);
            drop((route, source, workspace));
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
