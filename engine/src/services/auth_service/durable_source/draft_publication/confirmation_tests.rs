use super::*;
use std::future::ready;

#[tokio::test]
async fn exact_confirmations_advance_one_slot_then_complete_once_including_empty_tasks() {
    let fixture = Fixture::ready().await;
    for texts in [vec![], vec!["same", "same"]] {
        let mut attempt = fixture.attempt(&texts);
        for index in 0..texts.len() {
            let returned = revision(&attempt, index);
            assert_eq!(
                attempt
                    .await_artifact(index, ready(Ok(returned.clone())))
                    .await
                    .unwrap(),
                SessionDraftPublicationProgress::ArtifactConfirmed { index }
            );
            assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
            assert_eq!(attempt.confirmed_artifacts().len(), index + 1);
            assert_eq!(attempt.confirmed_artifacts()[index], returned);
            assert!(attempt.confirmed_task().is_none());
        }
        let task = task_snapshot(&attempt);
        assert_eq!(task.manifest.input_refs(), attempt.allocated_artifacts());
        assert_eq!(
            attempt
                .await_task(task.manifest.clone(), ready(Ok(task.clone())))
                .await
                .unwrap(),
            SessionDraftPublicationProgress::TaskConfirmed
        );
        assert_eq!(attempt.state(), &SessionDraftPublicationState::Complete);
        assert_eq!(attempt.confirmed_task(), Some(&task));
        assert_eq!(
            attempt.advance(&fixture.token).await,
            Err(SessionDraftPublicationError::Stopped)
        );
        assert_eq!(attempt.confirmed_task(), Some(&task));
    }
}

#[tokio::test]
async fn artifact_errors_preserve_the_prior_commit_and_never_allow_continuation() {
    let fixture = Fixture::ready().await;
    let mut attempt = fixture.attempt(&["first", "second"]);
    let first = revision(&attempt, 0);
    attempt
        .await_artifact(0, ready(Ok(first.clone())))
        .await
        .unwrap();
    let failure = SessionArtifactError::Artifact(
        crate::services::artifact_store::ArtifactStoreError::Conflict,
    );
    assert_eq!(
        attempt.await_artifact(1, ready(Err(failure))).await,
        Err(SessionDraftPublicationError::Artifact(failure))
    );
    assert_eq!(attempt.confirmed_artifacts(), &[first]);
    assert_eq!(
        attempt.state(),
        &SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Artifact {
            index: 1,
            reference: attempt.allocated_artifacts()[1].clone(),
        })
    );
    assert_eq!(
        attempt.advance(&fixture.token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
}

#[tokio::test]
async fn task_errors_preserve_all_confirmed_artifacts_and_the_task_target() {
    let fixture = Fixture::ready().await;
    let mut attempt = fixture.attempt(&["text"]);
    let first = revision(&attempt, 0);
    attempt
        .await_artifact(0, ready(Ok(first.clone())))
        .await
        .unwrap();
    let failure = SessionTaskError::Task(crate::services::task_store::TaskStoreError::Conflict);
    assert_eq!(
        attempt
            .await_task(attempt.publication_manifest().unwrap(), ready(Err(failure)))
            .await,
        Err(SessionDraftPublicationError::Task(failure))
    );
    assert_eq!(attempt.confirmed_artifacts(), &[first]);
    assert_eq!(
        attempt.state(),
        &SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Task {
            reference: attempt.task_ref()
        })
    );
    assert!(attempt.confirmed_task().is_none());
    assert_eq!(
        attempt.advance(&fixture.token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
}

#[tokio::test]
async fn artifact_confirmation_requires_every_expected_field_and_the_exact_preallocated_slot() {
    let fixture = Fixture::ready().await;
    for field in 0..12 {
        let mut attempt = fixture.attempt(&["same", "same"]);
        let mut returned = revision(&attempt, 0);
        match field {
            0 => returned.reference.artifact_id = uuid::Uuid::new_v4().try_into().unwrap(),
            1 => returned.reference.revision_id = uuid::Uuid::new_v4().try_into().unwrap(),
            2 => {
                returned.reference.workspace.workspace_id = uuid::Uuid::new_v4().try_into().unwrap()
            }
            3 => returned.reference.sha256 = "0".repeat(64).parse().unwrap(),
            4 => returned.owner.authority_id = uuid::Uuid::new_v4().try_into().unwrap(),
            5 => returned.byte_length += 1,
            6 => returned.sequence = 2.try_into().unwrap(),
            7 => returned.parent = Some(attempt.allocated_artifacts()[1].clone()),
            8 => returned.kind = "example.other".parse().unwrap(),
            9 => returned.title = "Unexpected title".into(),
            10 => returned.media_type = "application/octet-stream".into(),
            11 => returned = revision(&attempt, 1),
            _ => unreachable!(),
        }
        assert_eq!(
            attempt.await_artifact(0, ready(Ok(returned))).await,
            Err(SessionDraftPublicationError::InvalidConfirmation),
            "field {field}"
        );
        assert!(attempt.confirmed_artifacts().is_empty());
        assert!(matches!(
            attempt.state(),
            SessionDraftPublicationState::Unconfirmed(_)
        ));
        assert_eq!(
            attempt.advance(&fixture.token).await,
            Err(SessionDraftPublicationError::Stopped)
        );
    }
    // Schema version is a checked unit type, so no non-v1 confirmation can be constructed.
    assert!(serde_json::from_value::<CoreSchemaVersion>(json!(2)).is_err());
}

#[tokio::test]
async fn a_later_artifact_cannot_change_the_owner_derived_from_the_first_confirmation() {
    let fixture = Fixture::ready().await;
    let mut attempt = fixture.attempt(&["first", "second"]);
    let first = revision(&attempt, 0);
    attempt
        .await_artifact(0, ready(Ok(first.clone())))
        .await
        .unwrap();
    let mut second = revision(&attempt, 1);
    second.owner.principal_id = uuid::Uuid::new_v4().try_into().unwrap();
    assert_eq!(
        attempt.await_artifact(1, ready(Ok(second))).await,
        Err(SessionDraftPublicationError::InvalidConfirmation)
    );
    assert_eq!(attempt.confirmed_artifacts(), &[first]);
}

fn definition() -> TaskDefinition {
    let versioned = VersionedName {
        name: "example.metadata".parse().unwrap(),
        version: 1.try_into().unwrap(),
    };
    TaskDefinition {
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
    }
}

#[tokio::test]
async fn task_confirmation_requires_exact_manifest_reference_owner_and_manual_draft_revision_one() {
    let fixture = Fixture::ready().await;
    for field in 0..10 {
        let mut attempt = fixture.attempt(&["text"]);
        let first = revision(&attempt, 0);
        attempt
            .await_artifact(0, ready(Ok(first.clone())))
            .await
            .unwrap();
        let mut returned = task_snapshot(&attempt);
        let expected = returned.manifest.clone();
        match field {
            0 => returned.task.reference.task_id = uuid::Uuid::new_v4().try_into().unwrap(),
            1 => {
                returned.task.reference.workspace.workspace_id =
                    uuid::Uuid::new_v4().try_into().unwrap()
            }
            2 => returned.task.owner.authority_id = uuid::Uuid::new_v4().try_into().unwrap(),
            3 => returned.task.owner.principal_id = uuid::Uuid::new_v4().try_into().unwrap(),
            4 => returned.task.revision = 2.try_into().unwrap(),
            5 => returned.task.status = TaskStatus::Ready,
            6 => returned.task.definition = Some(definition()),
            7 => returned.task.title = "Unexpected task title".into(),
            8 => returned.task.input_refs.clear(),
            9 => {
                returned.manifest = TaskContentManifest::new(
                    expected.title(),
                    vec![TextPayloadBinding::new(
                        expected.input_refs()[0].clone(),
                        "different-label.txt",
                        "future.lang",
                    )
                    .unwrap()],
                )
                .unwrap()
            }
            _ => unreachable!(),
        }
        assert_eq!(
            attempt.await_task(expected, ready(Ok(returned))).await,
            Err(SessionDraftPublicationError::InvalidConfirmation),
            "field {field}"
        );
        assert_eq!(attempt.confirmed_artifacts(), &[first]);
        assert!(attempt.confirmed_task().is_none());
        assert_eq!(
            attempt.state(),
            &SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Task {
                reference: attempt.task_ref()
            })
        );
    }
}
