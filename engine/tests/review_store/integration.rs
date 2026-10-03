#![cfg(unix)]
use super::*;
use cyanrex_engine::services::{
    artifact_store::{ArtifactDraft, ArtifactStore},
    task_catalog::{TaskCatalog, TaskCatalogError, TaskProvider},
    task_store::{TaskDraft, TaskStore},
};

struct TextRules;
impl TaskProvider for TextRules {
    type Evidence<'a> = &'a [u8];
    fn package(&self) -> VersionedName {
        assessment().definition.package
    }
    fn definitions(&self) -> Vec<TaskDefinition> {
        let result = assessment();
        vec![TaskDefinition {
            schema_version: CoreSchemaVersion,
            reference: result.definition,
            title: "Review text".into(),
            summary: "A rule without a Run".into(),
            evidence_schema: result.evidence_schema,
            assessment_policy: result.policy,
        }]
    }
    fn assess(
        &self,
        _: &TaskDefinition,
        bytes: &[u8],
    ) -> Result<AssessmentOutcome, TaskCatalogError> {
        Ok(AssessmentOutcome {
            verdict: if bytes.starts_with(b"Reviewed") {
                AssessmentVerdict::Passed
            } else {
                AssessmentVerdict::NotPassed
            },
            feedback: vec!["Checked the exact bytes supplied by the fixture".into()],
        })
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_reviews_artifact_edits_keep_old_judgments_and_task_state() {
    let f = Fixture::ready().await;
    let url = std::env::var("CYANREX_TEST_DATABASE_URL").unwrap();
    let artifact_schema = format!("{}_artifacts", f.schema);
    let task_schema = format!("{}_tasks", f.schema);
    for schema in [&artifact_schema, &task_schema] {
        query(&format!("CREATE SCHEMA {schema}"))
            .execute(&f.admin)
            .await
            .unwrap();
    }
    let artifacts_pool = pool_for(&url, artifact_schema.clone(), 2).await;
    let tasks_pool = pool_for(&url, task_schema.clone(), 2).await;
    let directory =
        std::env::temp_dir().join(format!("cyanrex-review-text-{}", uuid::Uuid::new_v4()));
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let artifacts = ArtifactStore::open(artifacts_pool.clone(), scope(), &directory).unwrap();
    artifacts.install_empty_namespace().await.unwrap();
    let tasks = TaskStore::new(tasks_pool.clone(), scope());
    tasks.install_empty_namespace().await.unwrap();
    let content = |bytes: &[u8]| {
        ArtifactDraft::new(
            "sample.document".parse().unwrap(),
            "Plain text",
            "text/plain",
            bytes.to_vec(),
        )
        .unwrap()
    };
    let original = artifacts
        .create(id(), id(), reviewer(), content(b"Reviewed first text"))
        .await
        .unwrap();
    let bytes = artifacts
        .read(&original.reference, reviewer())
        .await
        .unwrap()
        .unwrap()
        .bytes;
    let catalog = TaskCatalog::new(TextRules).unwrap();
    let definition = assessment().definition;
    let mut task = tasks
        .create(
            id(),
            reviewer(),
            TaskDraft::from_catalog(
                &catalog,
                &definition,
                "Edit text",
                vec![original.reference.clone()],
            )
            .unwrap(),
        )
        .await
        .unwrap();
    for status in [
        TaskStatus::Ready,
        TaskStatus::InProgress,
        TaskStatus::InReview,
    ] {
        task = tasks
            .transition(task.reference, reviewer(), task.revision, status)
            .await
            .unwrap();
    }
    let automatic = f
        .store
        .create(
            id(),
            reviewer(),
            ReviewDraft::rule(
                vec![original.reference.clone()],
                vec![],
                catalog.assess(&definition, &bytes).unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let human = f
        .store
        .create(id(), reviewer(), draft(original.reference.clone()))
        .await
        .unwrap();
    // This fixture supplies trusted attribution; it does not simulate Session authorization.
    let new = artifacts
        .revise(
            &original.reference,
            id(),
            reviewer(),
            content(b"Different second text"),
        )
        .await
        .unwrap();
    for review in [automatic, human] {
        let saved = f
            .store
            .read(review.reference, reviewer())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.targets, vec![original.reference.clone()]);
        assert!(!saved.targets.contains(&new.reference));
        assert_eq!(
            artifacts
                .read(&saved.targets[0], reviewer())
                .await
                .unwrap()
                .unwrap()
                .bytes,
            bytes
        );
    }
    assert_eq!(
        tasks.get(task.reference, reviewer()).await.unwrap(),
        Some(task)
    );
    let new_bytes = artifacts
        .read(&new.reference, reviewer())
        .await
        .unwrap()
        .unwrap()
        .bytes;
    assert_eq!(
        catalog
            .assess(&definition, &new_bytes)
            .unwrap()
            .outcome
            .verdict,
        AssessmentVerdict::NotPassed
    );
    artifacts_pool.close().await;
    tasks_pool.close().await;
    for schema in [&artifact_schema, &task_schema] {
        query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&f.admin)
            .await
            .unwrap();
    }
    std::fs::remove_dir_all(directory).unwrap();
    f.cleanup().await;
}
