use super::super::inspection::{artifact_outcome, task_outcome};
use super::*;
use crate::services::auth_service::durable_source::{
    DurableAuthError, SessionCommandError, SessionTaskContent,
};
use std::future::ready;
use SessionDraftCheckpointObservedOutcome::{
    DiffersFromCheckpointMetadata as Differs, MatchesCheckpointMetadata as Matches, NotVisible,
};

fn parse_wire(wire: serde_json::Value) -> SessionDraftPublicationCheckpoint {
    SessionDraftPublicationCheckpoint::parse_json(&serde_json::to_vec(&wire).unwrap()).unwrap()
}

fn wire(checkpoint: &SessionDraftPublicationCheckpoint) -> serde_json::Value {
    serde_json::from_slice(&checkpoint.to_json().unwrap()).unwrap()
}

fn report_claim(
    attempt: &SessionTaskDraftPublication,
    state: &str,
    count: usize,
) -> SessionDraftPublicationCheckpoint {
    let mut wire: serde_json::Value =
        serde_json::from_slice(&attempt.checkpoint().unwrap().to_json().unwrap()).unwrap();
    wire["reported_state"] = json!(state);
    wire["reported_confirmed_artifact_count"] = json!(count);
    parse_wire(wire)
}

#[tokio::test]
async fn checkpoint_inspection_uses_existing_current_session_readers_after_the_attempt_is_dropped()
{
    let fixture = Fixture::ready().await;
    for texts in [vec![], vec!["original"]] {
        let attempt = fixture.attempt(&texts);
        let checkpoint = report_claim(&attempt, "unconfirmed", 0);
        let before = checkpoint.to_json().unwrap();
        drop(attempt);
        let expected = if texts.is_empty() {
            SessionDraftCheckpointInspectionError::Task(DurableAuthError::StorageUnavailable.into())
        } else {
            SessionDraftCheckpointInspectionError::Artifact(
                DurableAuthError::StorageUnavailable.into(),
            )
        };
        assert_eq!(
            fixture
                .source
                .inspect_task_draft_checkpoint(&fixture.token, &fixture.workspace, &checkpoint)
                .await,
            Err(expected)
        );
        assert_eq!(checkpoint.to_json().unwrap(), before);
    }
}

#[tokio::test]
async fn checkpoint_inspection_rejects_non_unknown_and_foreign_scopes_before_io() {
    let fixture = Fixture::ready().await;
    for texts in [vec![], vec!["original"]] {
        let attempt = fixture.attempt(&texts);
        for state in ["ready", "complete"] {
            let checkpoint = report_claim(&attempt, state, texts.len());
            assert_eq!(
                fixture
                    .source
                    .inspect_task_draft_checkpoint("invalid", &fixture.workspace, &checkpoint)
                    .await,
                Err(SessionDraftCheckpointInspectionError::NoUnconfirmedStep)
            );
        }
        let checkpoint = report_claim(&attempt, "unconfirmed", 0);
        for field in ["authority_id", "workspace_id"] {
            let mut changed = wire(&checkpoint);
            let foreign = json!(uuid::Uuid::new_v4().to_string());
            changed["task"]["workspace"][field] = foreign.clone();
            for item in changed["manifest"]["payload"].as_array_mut().unwrap() {
                item["artifact"]["workspace"][field] = foreign.clone();
            }
            let changed = parse_wire(changed);
            assert_eq!(
                fixture
                    .source
                    .inspect_task_draft_checkpoint(&fixture.token, &fixture.workspace, &changed)
                    .await,
                Err(SessionDraftCheckpointInspectionError::ScopeMismatch)
            );
        }
        let other = DurableAuthSource::new(
            fixture.source.pool.clone(),
            uuid::Uuid::new_v4().try_into().unwrap(),
        );
        assert_eq!(
            other
                .inspect_task_draft_checkpoint(&fixture.token, &fixture.workspace, &checkpoint)
                .await,
            Err(SessionDraftCheckpointInspectionError::ScopeMismatch)
        );
    }
}

#[tokio::test]
async fn checkpoint_inspection_accepts_a_new_token_only_through_the_reader_and_keeps_q_separate() {
    let fixture = Fixture::ready().await;
    for texts in [vec![], vec!["original"]] {
        let mut attempt = fixture.attempt(&texts);
        assert!(attempt.advance(&fixture.token).await.is_err());
        let checkpoint = attempt.checkpoint().unwrap();
        let bytes = checkpoint.to_json().unwrap();
        let state = attempt.state().clone();
        let fresh_token = uuid::Uuid::new_v4().to_string();
        drop(fixture.source.inspect_task_draft_checkpoint(
            &fresh_token,
            &fixture.workspace,
            &checkpoint,
        ));
        assert_eq!(checkpoint.to_json().unwrap(), bytes);
        let storage_error = if texts.is_empty() {
            SessionDraftCheckpointInspectionError::Task(DurableAuthError::StorageUnavailable.into())
        } else {
            SessionDraftCheckpointInspectionError::Artifact(
                DurableAuthError::StorageUnavailable.into(),
            )
        };
        assert_eq!(
            fixture
                .source
                .inspect_task_draft_checkpoint(&fresh_token, &fixture.workspace, &checkpoint)
                .await,
            Err(storage_error)
        );
        let invalid = if texts.is_empty() {
            SessionDraftCheckpointInspectionError::Task(SessionTaskError::Session(
                SessionCommandError::InvalidSession,
            ))
        } else {
            SessionDraftCheckpointInspectionError::Artifact(SessionArtifactError::Session(
                SessionCommandError::InvalidSession,
            ))
        };
        for token in ["invalid", "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"] {
            assert_eq!(
                fixture
                    .source
                    .inspect_task_draft_checkpoint(token, &fixture.workspace, &checkpoint)
                    .await,
                Err(invalid)
            );
        }
        assert_eq!(
            attempt.observe_unconfirmed(&fresh_token).await,
            Err(SessionDraftPublicationError::SessionChanged)
        );
        assert_eq!(
            attempt.advance(&fixture.token).await,
            Err(SessionDraftPublicationError::Stopped)
        );
        assert_eq!(attempt.state(), &state);
        assert!(attempt.confirmed_artifacts().is_empty());
        assert!(attempt.confirmed_task().is_none());
        assert_eq!(checkpoint.to_json().unwrap(), bytes);
        assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
    }
}

fn content(attempt: &SessionTaskDraftPublication, index: usize) -> ArtifactContent {
    ArtifactContent {
        revision: revision(attempt, index),
        bytes: attempt.plan.items()[index].text().as_bytes().to_vec(),
    }
}

#[tokio::test]
async fn checkpoint_artifact_comparison_requires_creation_metadata_and_actual_text_shape() {
    let fixture = Fixture::ready().await;
    let attempt = fixture.attempt(&["valid\r\n\t😀", "valid\r\n\t😀"]);
    let checkpoint = report_claim(&attempt, "unconfirmed", 0);
    let original = content(&attempt, 0);
    assert_eq!(artifact_outcome(&checkpoint, 0, None), NotVisible);
    assert_eq!(artifact_outcome(&checkpoint, 0, Some(&original)), Matches);
    assert_eq!(artifact_outcome(&checkpoint, 32, Some(&original)), Differs);
    for field in 0..11 {
        let mut changed = original.clone();
        match field {
            0 => changed.revision.reference = attempt.allocated_artifacts()[1].clone(),
            1 => changed.revision.owner.authority_id = uuid::Uuid::new_v4().try_into().unwrap(),
            2 => changed.revision.sequence = 2.try_into().unwrap(),
            3 => changed.revision.parent = Some(original.revision.reference.clone()),
            4 => changed.revision.kind = "example.other".parse().unwrap(),
            5 => changed.revision.title = "Changed title".into(),
            6 => changed.revision.media_type = "application/octet-stream".into(),
            7 => changed.revision.byte_length += 1,
            8 => changed.bytes = b"different".to_vec(),
            9 => changed.revision.reference.sha256 = "0".repeat(64).parse().unwrap(),
            10 => changed.bytes = vec![b'x'; MAX_TEXT_PAYLOAD_BYTES + 1],
            _ => unreachable!(),
        }
        assert_eq!(
            artifact_outcome(&checkpoint, 0, Some(&changed)),
            Differs,
            "field {field}"
        );
    }
    // A forged record can pin a well-formed generic binary Artifact. Matching length/digest
    // must not hide that the bytes could never pass C2-O's text contract.
    for bytes in [vec![0xff], vec![0], vec![0x7f], vec![0xc2, 0x85]] {
        let mut generic = original.clone();
        generic.bytes = bytes;
        generic.revision.byte_length = generic.bytes.len() as u64;
        generic.revision.reference.sha256 = format!("{:x}", Sha256::digest(&generic.bytes))
            .parse()
            .unwrap();
        let mut changed = wire(&checkpoint);
        changed["manifest"]["payload"][0]["artifact"]["sha256"] =
            json!(generic.revision.reference.sha256);
        changed["byte_lengths"][0] = json!(generic.revision.byte_length);
        assert_eq!(
            artifact_outcome(&parse_wire(changed), 0, Some(&generic)),
            Differs
        );
    }
    // Labels are not stored on Artifacts, and the record contains no original owner assertion.
    let mut changed = wire(&checkpoint);
    changed["manifest"]["payload"][0]["filename"] = json!("different.txt");
    changed["manifest"]["payload"][0]["language"] = json!("another.hint");
    let mut current_owner = original.clone();
    current_owner.revision.owner.principal_id = uuid::Uuid::new_v4().try_into().unwrap();
    assert_eq!(
        artifact_outcome(&parse_wire(changed), 0, Some(&current_owner)),
        Matches
    );
}

#[tokio::test]
async fn checkpoint_task_comparison_requires_current_full_manifest_and_ordered_owned_contents() {
    let fixture = Fixture::ready().await;
    let mut attempt = fixture.attempt(&["first", "second"]);
    for index in 0..2 {
        let returned = revision(&attempt, index);
        attempt
            .await_artifact(index, ready(Ok(returned)))
            .await
            .unwrap();
    }
    let checkpoint = report_claim(&attempt, "unconfirmed", 2);
    let original = SessionTaskContent {
        snapshot: task_snapshot(&attempt),
        contents: (0..2).map(|index| content(&attempt, index)).collect(),
    };
    assert_eq!(task_outcome(&checkpoint, None), NotVisible);
    assert_eq!(task_outcome(&checkpoint, Some(&original)), Matches);
    for field in 0..15 {
        let mut changed = original.clone();
        match field {
            0 => changed.snapshot.task.reference.task_id = uuid::Uuid::new_v4().try_into().unwrap(),
            1 => {
                changed.snapshot.task.owner.authority_id = uuid::Uuid::new_v4().try_into().unwrap()
            }
            2 => {
                let versioned = VersionedName {
                    name: "example.metadata".parse().unwrap(),
                    version: 1.try_into().unwrap(),
                };
                changed.snapshot.task.definition = Some(TaskDefinition {
                    schema_version: CoreSchemaVersion,
                    reference: TaskDefinitionRef {
                        package: versioned.clone(),
                        name: "example.task".parse().unwrap(),
                        version: 1.try_into().unwrap(),
                    },
                    title: "Definition".into(),
                    summary: "Metadata".into(),
                    evidence_schema: versioned.clone(),
                    assessment_policy: versioned,
                });
            }
            3 => changed.snapshot.task.status = TaskStatus::Ready,
            4 => changed.snapshot.task.revision = 2.try_into().unwrap(),
            5 => changed.snapshot.task.title = "Other title".into(),
            6 => changed.snapshot.task.input_refs.reverse(),
            7 => changed.contents.clear(),
            8 => changed.contents.reverse(),
            9 => {
                changed.contents[0].revision.owner.principal_id =
                    uuid::Uuid::new_v4().try_into().unwrap()
            }
            10 => changed.contents[0].revision.title = "Other artifact".into(),
            11 => changed.contents[0].bytes = b"other".to_vec(),
            12 => changed.contents[0].revision.byte_length += 1,
            13 => {
                changed.snapshot.manifest =
                    TaskContentManifest::new("Other manifest", vec![]).unwrap()
            }
            14 => changed.contents[0].revision.media_type = "text/html".into(),
            _ => unreachable!(),
        }
        assert_eq!(
            task_outcome(&checkpoint, Some(&changed)),
            Differs,
            "field {field}"
        );
    }
    let mut label_claim = wire(&checkpoint);
    label_claim["manifest"]["payload"][0]["filename"] = json!("other-label.txt");
    assert_eq!(
        task_outcome(&parse_wire(label_claim), Some(&original)),
        Differs
    );
    let mut length_claim = wire(&checkpoint);
    length_claim["byte_lengths"][0] = json!(0);
    assert_eq!(
        task_outcome(&parse_wire(length_claim), Some(&original)),
        Differs
    );
    assert_eq!(checkpoint.reported_confirmed_artifact_count(), 2);
    assert_eq!(attempt.confirmed_artifacts().len(), 2);
    assert!(attempt.confirmed_task().is_none());
}

#[tokio::test]
async fn checkpoint_empty_task_comparison_uses_scope_without_inventing_an_original_owner() {
    let fixture = Fixture::ready().await;
    let attempt = fixture.attempt(&[]);
    let checkpoint = report_claim(&attempt, "unconfirmed", 0);
    let bytes = checkpoint.to_json().unwrap();
    let mut current = SessionTaskContent {
        snapshot: task_snapshot(&attempt),
        contents: vec![],
    };
    current.snapshot.task.owner.principal_id = uuid::Uuid::new_v4().try_into().unwrap();
    assert_eq!(task_outcome(&checkpoint, Some(&current)), Matches);
    assert_eq!(task_outcome(&checkpoint, None), NotVisible);
    current.snapshot.task.owner.authority_id = uuid::Uuid::new_v4().try_into().unwrap();
    assert_eq!(task_outcome(&checkpoint, Some(&current)), Differs);
    assert_eq!(checkpoint.to_json().unwrap(), bytes);
    assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
    assert!(attempt.confirmed_task().is_none());
}
