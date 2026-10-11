use super::fixture::*;
use super::sql_fixture::*;
use super::*;
use cyanrex_engine::services::artifact_store::ArtifactStoreError;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_index_never_reads_artifact_bytes_or_artifact_namespace() {
    for mode in ["damaged", "missing", "namespace"] {
        let f = Fixture::ready().await;
        let token = &f.auth.learner_token;
        let artifact = f
            .create_artifact(token, "\tPrivate 原文😀\r\n".as_bytes())
            .await;
        let task = f.create(vec![artifact.reference.clone()]).await;
        let workspace = index(&f);
        let expected = list(&f, &workspace, token, None, None).await.unwrap();
        assert_eq!(expected.entries, vec![entry(&task)]);
        assert!(f
            .get(token, task.task.reference.task_id)
            .await
            .unwrap()
            .is_some());
        let error = if mode == "namespace" {
            assert_eq!(
                query("UPDATE collaboration_artifact_schema SET version = 999")
                    .execute(&f.artifact_pool)
                    .await
                    .unwrap()
                    .rows_affected(),
                1
            );
            SessionTaskError::Artifact(ArtifactStoreError::UnsupportedSchema)
        } else {
            let path = f.blob(&artifact.reference);
            if mode == "missing" {
                fs::remove_file(path).unwrap();
            } else {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                fs::write(&path, b"Effective synthetic damage").unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
            }
            SessionTaskError::Artifact(ArtifactStoreError::BlobUnavailable)
        };
        assert_eq!(
            list(&f, &workspace, token, None, None).await.unwrap(),
            expected,
            "{mode}"
        );
        let before = snapshot(&f).await;
        assert_eq!(
            f.get(token, task.task.reference.task_id).await,
            Err(error),
            "content reopening still revalidates {mode}"
        );
        assert_eq!(snapshot(&f).await, before);
        f.cleanup().await;
    }
}
