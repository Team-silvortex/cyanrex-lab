use super::*;
use std::future::ready;

fn artifact_content(attempt: &SessionTaskDraftPublication, index: usize) -> ArtifactContent {
    ArtifactContent {
        revision: revision(attempt, index),
        bytes: attempt.plan.items()[index].text().as_bytes().to_vec(),
    }
}

async fn unknown_artifact(f: &Fixture, texts: &[&str]) -> SessionTaskDraftPublication {
    let mut attempt = f.attempt(texts);
    assert!(matches!(
        attempt.advance(&f.token).await,
        Err(SessionDraftPublicationError::Artifact(_))
    ));
    attempt
}

async fn unknown_task(f: &Fixture, texts: &[&str]) -> SessionTaskDraftPublication {
    let mut attempt = f.attempt(texts);
    for index in 0..texts.len() {
        let confirmed = revision(&attempt, index);
        attempt
            .await_artifact(index, ready(Ok(confirmed)))
            .await
            .unwrap();
    }
    assert!(matches!(
        attempt.advance(&f.token).await,
        Err(SessionDraftPublicationError::Task(_))
    ));
    attempt
}

#[tokio::test]
async fn an_unconfirmed_observation_uses_the_existing_session_reader_without_changing_state() {
    let fixture = Fixture::ready().await;
    for texts in [vec![], vec!["text"]] {
        let mut attempt = fixture.attempt(&texts);
        let failure = attempt.advance(&fixture.token).await.unwrap_err();
        let state = attempt.state().clone();
        let observed = attempt.observe_unconfirmed(&fixture.token).await;
        assert_eq!(observed, Err(failure));
        assert_eq!(attempt.state(), &state);
        assert!(attempt.confirmed_artifacts().is_empty());
        assert!(attempt.confirmed_task().is_none());
        assert_eq!(
            attempt.advance(&fixture.token).await,
            Err(SessionDraftPublicationError::Stopped)
        );
    }
}

#[tokio::test]
async fn ready_and_complete_attempts_cannot_observe_even_with_a_changed_token() {
    let fixture = Fixture::ready().await;
    let mut attempt = fixture.attempt(&[]);
    for token in [&fixture.token, "invalid"] {
        assert_eq!(
            attempt.observe_unconfirmed(token).await,
            Err(SessionDraftPublicationError::NoUnconfirmedStep)
        );
    }
    assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
    let completed = task_snapshot(&attempt);
    attempt
        .await_task(completed.manifest.clone(), ready(Ok(completed.clone())))
        .await
        .unwrap();
    for token in [&fixture.token, "invalid"] {
        assert_eq!(
            attempt.observe_unconfirmed(token).await,
            Err(SessionDraftPublicationError::NoUnconfirmedStep)
        );
    }
    assert_eq!(attempt.state(), &SessionDraftPublicationState::Complete);
    assert_eq!(attempt.confirmed_task(), Some(&completed));
}

#[tokio::test]
async fn observation_rejects_other_or_noncanonical_tokens_without_poisoning_or_rebinding() {
    let fixture = Fixture::ready().await;
    let attempt = unknown_artifact(&fixture, &["text"]).await;
    let before = attempt.state().clone();
    for token in [
        "invalid".to_owned(),
        "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".to_owned(),
        uuid::Uuid::new_v4().to_string(),
    ] {
        assert_eq!(
            attempt.observe_unconfirmed(&token).await,
            Err(SessionDraftPublicationError::SessionChanged)
        );
        assert_eq!(attempt.state(), &before);
    }
    assert!(matches!(
        attempt.observe_unconfirmed(&fixture.token).await,
        Err(SessionDraftPublicationError::Artifact(_))
    ));
    assert_eq!(attempt.state(), &before);
    assert!(attempt.confirmed_artifacts().is_empty());
}

#[tokio::test]
async fn an_unpolled_observation_does_not_touch_the_uncertain_step_or_receipts() {
    let fixture = Fixture::ready().await;
    let attempt = unknown_task(&fixture, &["confirmed"]).await;
    let before = attempt.state().clone();
    let receipts = attempt.confirmed_artifacts().to_vec();
    drop(attempt.observe_unconfirmed(&fixture.token));
    assert_eq!(attempt.state(), &before);
    assert_eq!(attempt.confirmed_artifacts(), receipts);
    assert!(attempt.confirmed_task().is_none());
}

#[tokio::test]
async fn artifact_observation_compares_actual_bytes_and_exact_metadata_without_recording_confirmation(
) {
    let fixture = Fixture::ready().await;
    let mut attempt = unknown_artifact(&fixture, &["exact text", "exact text"]).await;
    let state = attempt.state().clone();
    let matching = artifact_content(&attempt, 0);
    assert_eq!(
        attempt.artifact_observation(0, Some(&matching)),
        SessionDraftObservedOutcome::MatchesPlannedCreate
    );
    assert_eq!(
        attempt.artifact_observation(0, None),
        SessionDraftObservedOutcome::NotVisible
    );
    for field in 0..10 {
        let mut changed = matching.clone();
        match field {
            0 => changed.bytes = b"other text".to_vec(),
            1 => changed.revision.reference = attempt.allocated_artifacts()[1].clone(),
            2 => changed.revision.byte_length += 1,
            3 => changed.revision.title = "Other title".into(),
            4 => changed.revision.kind = "example.other".parse().unwrap(),
            5 => changed.revision.media_type = "application/octet-stream".into(),
            6 => changed.revision.sequence = 2.try_into().unwrap(),
            7 => changed.revision.parent = Some(matching.revision.reference.clone()),
            8 => changed.revision.owner.authority_id = uuid::Uuid::new_v4().try_into().unwrap(),
            9 => changed.revision.reference.sha256 = "0".repeat(64).parse().unwrap(),
            _ => unreachable!(),
        }
        assert_eq!(
            attempt.artifact_observation(0, Some(&changed)),
            SessionDraftObservedOutcome::DiffersFromPlannedCreate,
            "field {field}"
        );
    }
    assert_eq!(attempt.state(), &state);
    assert!(attempt.confirmed_artifacts().is_empty());
    assert!(attempt.confirmed_task().is_none());
    assert_eq!(
        attempt.advance(&fixture.token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
}

#[tokio::test]
async fn task_observation_requires_actual_ordered_contents_and_the_original_creation_shape() {
    let fixture = Fixture::ready().await;
    let mut attempt = unknown_task(&fixture, &["first", "second"]).await;
    let state = attempt.state().clone();
    let receipts = attempt.confirmed_artifacts().to_vec();
    let snapshot = task_snapshot(&attempt);
    let expected = snapshot.manifest.clone();
    let matching = crate::services::auth_service::durable_source::SessionTaskContent {
        snapshot,
        contents: (0..2)
            .map(|index| artifact_content(&attempt, index))
            .collect(),
    };
    assert_eq!(
        attempt.task_observation(&expected, Some(&matching)),
        SessionDraftObservedOutcome::MatchesPlannedCreate
    );
    assert_eq!(
        attempt.task_observation(&expected, None),
        SessionDraftObservedOutcome::NotVisible
    );
    for field in 0..11 {
        let mut changed = matching.clone();
        match field {
            0 => changed.contents.clear(),
            1 => changed.contents.reverse(),
            2 => changed.contents[0].bytes = b"other".to_vec(),
            3 => changed.contents[0].revision.title = "Other artifact title".into(),
            4 => changed.snapshot.task.revision = 2.try_into().unwrap(),
            5 => changed.snapshot.task.status = TaskStatus::Ready,
            6 => changed.snapshot.task.reference.task_id = uuid::Uuid::new_v4().try_into().unwrap(),
            7 => {
                changed.snapshot.task.owner.principal_id = uuid::Uuid::new_v4().try_into().unwrap()
            }
            8 => changed.snapshot.task.title = "Other task title".into(),
            9 => changed.snapshot.task.input_refs.reverse(),
            10 => {
                changed.snapshot.manifest =
                    TaskContentManifest::new(expected.title(), vec![]).unwrap()
            }
            _ => unreachable!(),
        }
        assert_eq!(
            attempt.task_observation(&expected, Some(&changed)),
            SessionDraftObservedOutcome::DiffersFromPlannedCreate,
            "field {field}"
        );
    }
    assert_eq!(attempt.state(), &state);
    assert_eq!(attempt.confirmed_artifacts(), receipts);
    assert!(attempt.confirmed_task().is_none());
    assert_eq!(
        attempt.advance(&fixture.token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
}

#[tokio::test]
async fn empty_task_observation_matches_without_inventing_an_artifact_or_confirmed_owner() {
    let fixture = Fixture::ready().await;
    let attempt = unknown_task(&fixture, &[]).await;
    let expected = attempt.publication_manifest().unwrap();
    let matching = crate::services::auth_service::durable_source::SessionTaskContent {
        snapshot: task_snapshot(&attempt),
        contents: vec![],
    };
    assert_eq!(
        attempt.task_observation(&expected, Some(&matching)),
        SessionDraftObservedOutcome::MatchesPlannedCreate
    );
    assert_eq!(
        attempt.task_observation(&expected, None),
        SessionDraftObservedOutcome::NotVisible
    );
    assert!(attempt.confirmed_artifacts().is_empty());
    assert!(attempt.confirmed_task().is_none());
    assert!(matches!(
        attempt.state(),
        SessionDraftPublicationState::Unconfirmed(_)
    ));
}
