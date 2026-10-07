use super::*;
use std::future::ready;

fn check_report(
    attempt: &SessionTaskDraftPublication,
    expected_state: SessionDraftCheckpointState,
    count: usize,
    unknown: Option<SessionDraftPublicationStep>,
) -> Vec<u8> {
    let state = attempt.state().clone();
    let confirmed = attempt.confirmed_artifacts().to_vec();
    let completed = attempt.confirmed_task().cloned();
    let checkpoint = attempt.checkpoint().unwrap();
    assert_eq!(checkpoint.task_ref(), attempt.task_ref());
    assert_eq!(checkpoint.manifest().title(), attempt.plan.title());
    assert_eq!(
        checkpoint.manifest().input_refs(),
        attempt.allocated_artifacts()
    );
    assert_eq!(checkpoint.reported_state(), expected_state);
    assert_eq!(checkpoint.reported_confirmed_artifact_count(), count);
    assert_eq!(checkpoint.reported_unconfirmed_step(), unknown);
    for ((binding, item), length) in checkpoint
        .manifest()
        .payload()
        .iter()
        .zip(attempt.plan.items())
        .zip(checkpoint.byte_lengths())
    {
        assert_eq!(binding.filename(), item.filename());
        assert_eq!(binding.language(), item.language());
        assert_eq!(*length, item.text().len() as u64);
    }
    let bytes = checkpoint.to_json().unwrap();
    assert!(bytes.len() <= MAX_SESSION_DRAFT_CHECKPOINT_BYTES);
    let parsed = SessionDraftPublicationCheckpoint::parse_json(&bytes).unwrap();
    assert_eq!(parsed.to_json().unwrap(), bytes);
    assert_eq!(parsed.reported_state(), expected_state);
    assert_eq!(parsed.reported_confirmed_artifact_count(), count);
    assert_eq!(parsed.reported_unconfirmed_step(), unknown);
    assert_eq!(attempt.state(), &state);
    assert_eq!(attempt.confirmed_artifacts(), confirmed);
    assert_eq!(attempt.confirmed_task(), completed.as_ref());
    bytes
}

#[tokio::test]
async fn ready_checkpoints_export_without_io_or_changing_the_attempt() {
    let fixture = Fixture::ready().await;
    for texts in [
        vec![],
        vec!["private original text", "private original text"],
    ] {
        let attempt = fixture.attempt(&texts);
        assert!(attempt.checkpoint().is_ok());
        let bytes = check_report(&attempt, SessionDraftCheckpointState::Ready, 0, None);
        let wire = String::from_utf8(bytes).unwrap();
        for private in [
            "private original text",
            fixture.token.as_str(),
            "fingerprint",
            "owner",
            "created_at",
            "private_artifacts",
            "private_tasks",
            fixture.root.to_str().unwrap(),
            "synthetic@",
        ] {
            assert!(!wire.contains(private));
        }
        if texts.len() == 2 {
            let refs = attempt.checkpoint().unwrap().manifest().input_refs();
            assert_ne!(refs[0].artifact_id, refs[1].artifact_id);
            assert_eq!(refs[0].sha256, refs[1].sha256);
        }
        assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
        assert!(attempt.confirmed_artifacts().is_empty());
        assert!(attempt.confirmed_task().is_none());
        assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
    }
}

#[tokio::test]
async fn ready_prefix_reports_only_acknowledged_count_and_keeps_all_allocated_targets() {
    let fixture = Fixture::ready().await;
    let mut attempt = fixture.attempt(&["first", "second"]);
    for index in 0..2 {
        let returned = revision(&attempt, index);
        attempt
            .await_artifact(index, ready(Ok(returned)))
            .await
            .unwrap();
        check_report(
            &attempt,
            SessionDraftCheckpointState::Ready,
            index + 1,
            None,
        );
    }
    assert!(attempt.confirmed_task().is_none());
}

#[tokio::test]
async fn unconfirmed_artifact_checkpoint_derives_the_exact_next_slot_without_adopting_it() {
    let fixture = Fixture::ready().await;
    let mut attempt = fixture.attempt(&["first", "second"]);
    let returned = revision(&attempt, 0);
    attempt
        .await_artifact(0, ready(Ok(returned)))
        .await
        .unwrap();
    assert!(matches!(
        attempt.advance(&fixture.token).await,
        Err(SessionDraftPublicationError::Artifact(_))
    ));
    check_report(
        &attempt,
        SessionDraftCheckpointState::Unconfirmed,
        1,
        Some(SessionDraftPublicationStep::Artifact {
            index: 1,
            reference: attempt.allocated_artifacts()[1].clone(),
        }),
    );
    assert_eq!(
        attempt.advance(&fixture.token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
}

#[tokio::test]
async fn unconfirmed_task_checkpoint_preserves_full_prefix_including_named_empty_tasks() {
    let fixture = Fixture::ready().await;
    for texts in [vec![], vec!["original"]] {
        let mut attempt = fixture.attempt(&texts);
        for index in 0..texts.len() {
            let returned = revision(&attempt, index);
            attempt
                .await_artifact(index, ready(Ok(returned)))
                .await
                .unwrap();
        }
        assert!(matches!(
            attempt.advance(&fixture.token).await,
            Err(SessionDraftPublicationError::Task(_))
        ));
        check_report(
            &attempt,
            SessionDraftCheckpointState::Unconfirmed,
            texts.len(),
            Some(SessionDraftPublicationStep::Task {
                reference: attempt.task_ref(),
            }),
        );
        assert_eq!(
            attempt.advance(&fixture.token).await,
            Err(SessionDraftPublicationError::Stopped)
        );
    }
}

#[tokio::test]
async fn complete_checkpoint_reports_completion_without_exporting_task_or_artifact_receipts() {
    let fixture = Fixture::ready().await;
    for texts in [vec![], vec!["original"]] {
        let mut attempt = fixture.attempt(&texts);
        for index in 0..texts.len() {
            let returned = revision(&attempt, index);
            attempt
                .await_artifact(index, ready(Ok(returned)))
                .await
                .unwrap();
        }
        let task = task_snapshot(&attempt);
        attempt
            .await_task(task.manifest.clone(), ready(Ok(task)))
            .await
            .unwrap();
        let bytes = check_report(
            &attempt,
            SessionDraftCheckpointState::Complete,
            texts.len(),
            None,
        );
        let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(wire["task"].as_object().unwrap().len(), 2);
        for field in [
            "owner",
            "created_at",
            "updated_at",
            "confirmed_task",
            "confirmed_artifacts",
        ] {
            assert!(!String::from_utf8_lossy(&bytes).contains(field));
        }
        assert_eq!(
            attempt.advance(&fixture.token).await,
            Err(SessionDraftPublicationError::Stopped)
        );
    }
}

#[tokio::test]
async fn export_refuses_inconsistent_private_state_without_repairing_or_changing_it() {
    let fixture = Fixture::ready().await;
    for defect in 0..6 {
        let mut attempt = fixture.attempt(&["original"]);
        match defect {
            0 => {
                attempt.allocated.clear();
            }
            1 => {
                attempt.state = SessionDraftPublicationState::Complete;
            }
            2 => {
                attempt.state =
                    SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Task {
                        reference: attempt.task_ref(),
                    });
            }
            3 => {
                attempt.state = SessionDraftPublicationState::Unconfirmed(
                    SessionDraftPublicationStep::Artifact {
                        index: 1,
                        reference: attempt.allocated[0].clone(),
                    },
                );
            }
            4 => {
                let mut returned = revision(&attempt, 0);
                returned.byte_length += 1;
                attempt.confirmed.push(returned);
            }
            5 => {
                let returned = revision(&attempt, 0);
                attempt.confirmed.push(returned);
                attempt.completed = Some(task_snapshot(&attempt));
            }
            _ => unreachable!(),
        }
        let before = attempt.state().clone();
        let count = attempt.confirmed_artifacts().len();
        assert!(attempt.checkpoint().is_err(), "defect {defect}");
        assert_eq!(attempt.state(), &before);
        assert_eq!(attempt.confirmed_artifacts().len(), count);
    }
}
