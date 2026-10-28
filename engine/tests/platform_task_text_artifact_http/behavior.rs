use super::fixture::*;
use axum::{body::Body, http::StatusCode};
use cyanrex_engine::models::collaboration::{ArtifactRevision, MAX_TEXT_PAYLOAD_BYTES};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_http_commits_private_bytes_without_task_mutation() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let old = p
                .task_manifest(
                    10,
                    1,
                    manifest(
                        "协作材料，不是代码任务",
                        vec![p.reference(777, b"Old bytes are deliberately unmaterialized")],
                    ),
                )
                .await;
            let route = router(&p, 2).await;
            let files = p.files();
            let private = p.non_artifacts().await;
            for (index, text) in [
                String::new(),
                "협업 수업 자료😀\r\nDocument payload\t\n".into(),
                "x".repeat(MAX_TEXT_PAYLOAD_BYTES),
            ]
            .iter()
            .enumerate()
            {
                let n = index as u128 + 1;
                let raw = payload(&old, n, text).to_string();
                // The actual wire bound is inclusive, independent of the decoded text budget.
                let body = if index == 0 {
                    Body::from(format!("{raw}{}", " ".repeat(2 * 1024 * 1024 - raw.len())))
                } else {
                    Body::from(raw)
                };
                let reply = call(
                    &route,
                    request(&p, 1, "POST", &path(&old), body),
                    StatusCode::CREATED,
                )
                .await;
                assert_eq!(reply.as_object().unwrap().len(), 2);
                assert_eq!(reply["schema_version"], 1);
                assert!(
                    reply.get("text").is_none()
                        && reply.get("task").is_none()
                        && reply.get("manifest").is_none()
                );
                let artifact: ArtifactRevision =
                    serde_json::from_value(reply["artifact"].clone()).unwrap();
                assert_eq!(artifact.owner, p.owners[1]);
                assert_eq!(artifact.title, old.task.title);
                assert_eq!(artifact.reference, p.reference(n, text.as_bytes()));
                assert_eq!(artifact.kind.as_str(), "cyanrex.task-text");
                assert_eq!(artifact.media_type, "text/plain");
                assert_eq!(artifact.byte_length, text.len() as u64);
                assert_eq!(u64::from(artifact.sequence), 1);
                assert!(artifact.parent.is_none());
                p.read_committed(&artifact, text.as_bytes()).await;
                p.unchanged(&old).await;
                assert_eq!(private, p.non_artifacts().await);
                p.counts(index as i64 + 1).await;
                p.old_files_unchanged(&files, index + 1);
            }
            for absent in [
                "/platform/v1/text-artifacts".into(),
                "/platform/v1/tasks".into(),
                format!("/platform/v1/tasks/{}/content", old.task.reference.task_id),
            ] {
                error(
                    &route,
                    request(&p, 1, "POST", &absent, unread()),
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "not_attempted",
                )
                .await;
            }
            drop(route);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}
