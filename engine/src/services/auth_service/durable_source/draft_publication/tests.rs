use super::*;
use crate::models::collaboration::*;
use chrono::{TimeZone, Utc};
use serde_json::json;
use sqlx_postgres::PgPoolOptions;
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};

#[path = "cancellation_tests.rs"]
mod cancellation;
#[path = "checkpoint_tests.rs"]
mod checkpoint;
#[path = "confirmation_tests.rs"]
mod confirmations;
#[path = "inspection_tests.rs"]
mod inspection;
#[path = "observation_tests.rs"]
mod observation;

struct Fixture {
    source: DurableAuthSource,
    workspace: SessionTaskContentWorkspace,
    root: PathBuf,
    token: String,
}
impl Fixture {
    async fn ready() -> Self {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://synthetic@127.0.0.1:1/unused")
            .unwrap();
        pool.close().await;
        let scope = WorkspaceRef {
            authority_id: uuid::Uuid::new_v4().try_into().unwrap(),
            workspace_id: uuid::Uuid::new_v4().try_into().unwrap(),
        };
        let source = DurableAuthSource::new(pool, scope.authority_id);
        let root = std::env::temp_dir().join(format!(
            "cyanrex-draft-publication-{}",
            uuid::Uuid::new_v4()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let artifacts = source
            .open_artifact_workspace("private_artifacts", scope, &root)
            .unwrap();
        let workspace =
            SessionTaskContentWorkspace::new("private_tasks", scope, artifacts).unwrap();
        Self {
            source,
            workspace,
            root,
            token: uuid::Uuid::new_v4().to_string(),
        }
    }

    fn attempt(&self, texts: &[&str]) -> SessionTaskDraftPublication {
        self.source
            .prepare_task_draft_publication(&self.token, self.workspace.clone(), text_plan(texts))
            .ok()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only the exact empty directory created by this fixture; no blob was published.
        fs::remove_dir(&self.root).unwrap();
    }
}

fn plan() -> TaskDraftPublicationPlan {
    TaskDraftPublicationPlan::parse_json(
        br#"{"format":"cyanrex.task-draft","version":1,"title":"Named task","payload":[]}"#,
    )
    .ok()
    .unwrap()
}

fn text_plan(texts: &[&str]) -> TaskDraftPublicationPlan {
    let payload: Vec<_> = texts
        .iter()
        .enumerate()
        .map(|(index, text)| {
            json!({
                "id":format!("local-{index}"),"revision":9007199254740991_u64,
                "kind":"text","filename":"note.txt","language":"future.lang","text":text,
            })
        })
        .collect();
    TaskDraftPublicationPlan::parse_json(
        &serde_json::to_vec(&json!({
            "format":"cyanrex.task-draft","version":1,"title":"Named task","payload":payload,
        }))
        .unwrap(),
    )
    .ok()
    .unwrap()
}

fn owner(attempt: &SessionTaskDraftPublication) -> PrincipalRef {
    PrincipalRef {
        authority_id: attempt.task_ref().workspace.authority_id,
        principal_id: uuid::Uuid::from_u128(33).try_into().unwrap(),
    }
}

fn revision(attempt: &SessionTaskDraftPublication, index: usize) -> ArtifactRevision {
    ArtifactRevision {
        schema_version: CoreSchemaVersion,
        reference: attempt.allocated_artifacts()[index].clone(),
        owner: owner(attempt),
        sequence: 1.try_into().unwrap(),
        parent: None,
        kind: "cyanrex.task-text".parse().unwrap(),
        title: attempt.plan.title().into(),
        media_type: "text/plain".into(),
        byte_length: attempt.plan.items()[index].text().len() as u64,
        created_at: Utc.timestamp_opt(0, 0).unwrap(),
    }
}

fn task_snapshot(attempt: &SessionTaskDraftPublication) -> TaskContentSnapshot {
    let manifest = attempt.publication_manifest().unwrap();
    TaskContentSnapshot {
        task: TaskSnapshot {
            schema_version: CoreSchemaVersion,
            reference: attempt.task_ref(),
            owner: owner(attempt),
            title: manifest.title().into(),
            definition: None,
            input_refs: manifest.input_refs(),
            status: TaskStatus::Draft,
            revision: 1.try_into().unwrap(),
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(0, 0).unwrap(),
        },
        manifest,
    }
}

#[tokio::test]
async fn preparing_a_named_empty_draft_succeeds_without_accessing_the_closed_pool() {
    let fixture = Fixture::ready().await;
    assert!(fixture
        .source
        .prepare_task_draft_publication(&fixture.token, fixture.workspace.clone(), plan())
        .is_ok());
}

#[tokio::test]
async fn preparation_requires_canonical_token_and_matching_source_authority_without_io() {
    let fixture = Fixture::ready().await;
    for token in [
        "",
        "not-a-token",
        "00000000-0000-0000-0000-000000000000",
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
    ] {
        assert!(matches!(
            fixture
                .source
                .prepare_task_draft_publication(token, fixture.workspace.clone(), plan()),
            Err(SessionDraftPublicationError::InvalidPreparation)
        ));
    }
    let foreign = DurableAuthSource::new(
        fixture.source.pool.clone(),
        uuid::Uuid::new_v4().try_into().unwrap(),
    );
    assert!(matches!(
        foreign.prepare_task_draft_publication(&fixture.token, fixture.workspace.clone(), plan()),
        Err(SessionDraftPublicationError::InvalidPreparation)
    ));
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
}

#[tokio::test]
async fn preparation_allocates_every_unique_uuid4_and_digest_before_dispatch() {
    let fixture = Fixture::ready().await;
    let texts = vec!["equal text\r\n"; 32];
    let first = fixture.attempt(&texts);
    let second = fixture.attempt(&texts);
    assert_eq!(first.state(), &SessionDraftPublicationState::Ready);
    assert_eq!(first.task_ref().task_id.as_uuid().get_version_num(), 4);
    assert_ne!(first.task_ref(), second.task_ref());
    assert_eq!(first.allocated_artifacts().len(), 32);
    assert!(first.confirmed_artifacts().is_empty());
    assert!(first.confirmed_task().is_none());
    let mut artifacts = std::collections::HashSet::new();
    let mut revisions = std::collections::HashSet::new();
    for (reference, second) in first
        .allocated_artifacts()
        .iter()
        .zip(second.allocated_artifacts())
    {
        assert_eq!(reference.workspace, first.task_ref().workspace);
        assert_eq!(reference.artifact_id.as_uuid().get_version_num(), 4);
        assert_eq!(reference.revision_id.as_uuid().get_version_num(), 4);
        assert!(artifacts.insert(reference.artifact_id));
        assert!(revisions.insert(reference.revision_id));
        assert_ne!(reference, second);
        assert_eq!(
            reference.sha256.as_str(),
            format!("{:x}", Sha256::digest(texts[0].as_bytes()))
        );
    }
    assert!(fixture.attempt(&[]).allocated_artifacts().is_empty());
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
}

#[tokio::test]
async fn another_token_never_dispatches_or_poisons_the_ready_attempt() {
    let fixture = Fixture::ready().await;
    let mut attempt = fixture.attempt(&["text"]);
    for token in ["invalid".to_owned(), uuid::Uuid::new_v4().to_string()] {
        assert_eq!(
            attempt.advance(&token).await,
            Err(SessionDraftPublicationError::SessionChanged)
        );
        assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
        assert!(attempt.confirmed_artifacts().is_empty());
    }
    // The original token reaches the closed pool rather than being falsely treated as authority.
    assert!(matches!(
        attempt.advance(&fixture.token).await,
        Err(SessionDraftPublicationError::Artifact(_))
    ));
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
}

#[tokio::test]
async fn a_closed_pool_keeps_artifact_and_empty_task_steps_terminally_unconfirmed() {
    let fixture = Fixture::ready().await;
    for texts in [vec![], vec!["text"]] {
        let mut attempt = fixture.attempt(&texts);
        let expected = if texts.is_empty() {
            SessionDraftPublicationStep::Task {
                reference: attempt.task_ref(),
            }
        } else {
            SessionDraftPublicationStep::Artifact {
                index: 0,
                reference: attempt.allocated_artifacts()[0].clone(),
            }
        };
        let error = attempt.advance(&fixture.token).await.unwrap_err();
        assert!(matches!(
            error,
            SessionDraftPublicationError::Artifact(_) | SessionDraftPublicationError::Task(_)
        ));
        assert_eq!(
            attempt.state(),
            &SessionDraftPublicationState::Unconfirmed(expected)
        );
        assert!(attempt.confirmed_artifacts().is_empty());
        assert!(attempt.confirmed_task().is_none());
        assert_eq!(
            attempt.advance(&fixture.token).await,
            Err(SessionDraftPublicationError::Stopped)
        );
    }
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
}
