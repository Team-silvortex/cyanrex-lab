use super::fixture::*;
use cyanrex_engine::{
    models::collaboration::*,
    services::{
        artifact_store::ArtifactStoreError, auth_service::durable_source::SessionTaskError,
        task_store::TaskStoreError,
    },
};

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_cross_owner_missing_and_scope_never_create_files() {
    Fixture::checked(|f| {
        Box::pin(async move {
            let p = Prepared::new(f).await;
            let old = p.task(10, 0, "Owner-private baseline").await;
            let connector = p.connector().await;
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
            p.reject(&connector, 1, &old, 1, TaskStoreError::NotFound.into())
                .await;
            let mut forged_owner = old.clone();
            forged_owner.task.owner = p.owners[1];
            p.reject(
                &connector,
                1,
                &forged_owner,
                2,
                TaskStoreError::NotFound.into(),
            )
            .await;
            let mut missing = old.clone();
            missing.task.reference.task_id = task_id(999);
            p.reject(&connector, 0, &missing, 3, TaskStoreError::NotFound.into())
                .await;
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
            let mut foreign = old.clone();
            foreign.task.reference.workspace.workspace_id =
                uuid::Uuid::new_v4().try_into().unwrap();
            p.reject(
                &connector,
                0,
                &foreign,
                4,
                TaskStoreError::ScopeMismatch.into(),
            )
            .await;
            let before = snapshot(f).await;
            let files = p.files();
            assert!(p.root.swap());
            // A bad request must be rejected before even observing this deliberately wrong root.
            assert_eq!(
                connector
                    .create_session_task_text_artifact(
                        "invalid",
                        old.clone(),
                        artifact_id(10),
                        revision_id(10),
                        "Valid text".into()
                    )
                    .await,
                Err(invalid_session())
            );
            for text in [
                "x".repeat(MAX_TEXT_PAYLOAD_BYTES + 1),
                "NUL\0".into(),
                "DEL\u{7f}".into(),
                "ESC\u{1b}".into(),
            ] {
                assert_eq!(
                    p.create(&connector, 0, &old, 10, &text).await,
                    Err(SessionTaskError::Artifact(ArtifactStoreError::InvalidInput))
                );
            }
            let mut catalog = old.clone();
            catalog.task.definition = Some(TaskDefinition {
                schema_version: CoreSchemaVersion,
                reference: TaskDefinitionRef {
                    package: VersionedName {
                        name: "example.documents".parse().unwrap(),
                        version: 1.try_into().unwrap(),
                    },
                    name: "example.documents.notes".parse().unwrap(),
                    version: 1.try_into().unwrap(),
                },
                title: "Explicit catalogue work".into(),
                summary: "No implicit admission".into(),
                evidence_schema: None,
                assessment_policy: None,
            });
            assert_eq!(
                p.create(&connector, 0, &catalog, 10, "Valid text").await,
                Err(SessionTaskError::UnsupportedTask)
            );
            assert!(p.root.restore());
            assert_eq!(before, snapshot(f).await);
            assert_eq!(files, p.files());
            p.unchanged(&old).await;
            p.counts(0).await;
            drop(connector);
            assert!(super::storage_helpers::no_connections(f).await);
            assert!(p.finish());
        })
    })
    .await;
}

#[tokio::test]
#[ignore = "requires an explicit disposable CYANREX_TEST_DATABASE_URL with database and role administration"]
async fn postgres_platform_task_text_artifact_stale_tampered_status_and_revoked_reject_before_publication(
) {
    for mode in [
        "stale",
        "tampered-title",
        "tampered-manifest",
        "tampered-coherent-title",
        "created-at",
        "updated-at",
        "ready",
        "revoked",
        "outbox",
    ] {
        Fixture::checked(|f| {
            Box::pin(async move {
                let p = Prepared::new(f).await;
                let old = p.task(10, 0, "Exact original Task").await;
                let connector = p.connector().await;
                let mut expected = old.clone();
                let error = match mode {
                    "stale" => {
                        expected.task.revision = 2.try_into().unwrap();
                        TaskStoreError::StaleRevision.into()
                    }
                    "tampered-title" => {
                        expected.task.title = "Forged title".into();
                        TaskStoreError::InvalidInput.into()
                    }
                    "tampered-manifest" => {
                        expected.manifest = manifest("Forged manifest", vec![]);
                        TaskStoreError::InvalidInput.into()
                    }
                    "tampered-coherent-title" => {
                        expected.task.title = "Consistent but uncommitted title".into();
                        expected.manifest = manifest(&expected.task.title, vec![]);
                        TaskStoreError::InvalidInput.into()
                    }
                    "created-at" => {
                        expected.task.created_at -= chrono::Duration::seconds(1);
                        TaskStoreError::InvalidInput.into()
                    }
                    "updated-at" => {
                        expected.task.updated_at += chrono::Duration::seconds(1);
                        TaskStoreError::InvalidInput.into()
                    }
                    "ready" => {
                        expected = p.ready(&old).await;
                        TaskStoreError::InvalidTransition.into()
                    }
                    "revoked" => {
                        f.sql(&format!(
                            "DELETE FROM {}.sessions WHERE token='{}'",
                            f.source,
                            token_hash(&p.tokens[0])
                        ))
                        .await;
                        invalid_session()
                    }
                    _ => {
                        f.sql(&format!("DELETE FROM {}.collaboration_task_outbox", f.task))
                            .await;
                        TaskStoreError::InvalidRecord.into()
                    }
                };
                p.reject(&connector, 0, &expected, 1, error).await;
                p.counts(0).await;
                if mode != "outbox" {
                    p.unchanged(if mode == "ready" { &expected } else { &old })
                        .await;
                }
                drop(connector);
                assert!(super::storage_helpers::no_connections(f).await);
                assert!(p.finish());
            })
        })
        .await;
    }
}
