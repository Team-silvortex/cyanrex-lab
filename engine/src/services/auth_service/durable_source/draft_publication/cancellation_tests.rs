use super::*;
use std::{
    future::{pending, poll_fn, ready, Future},
    task::Poll,
};

#[tokio::test]
async fn dropping_an_unpolled_advance_never_marks_or_dispatches_a_step() {
    let fixture = Fixture::ready().await;
    for texts in [vec![], vec!["text"]] {
        let mut attempt = fixture.attempt(&texts);
        drop(attempt.advance(&fixture.token));
        assert_eq!(attempt.state(), &SessionDraftPublicationState::Ready);
        assert!(attempt.confirmed_artifacts().is_empty());
        assert!(attempt.confirmed_task().is_none());
    }
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
}

#[tokio::test]
async fn cancelling_a_polled_artifact_step_retains_the_confirmed_prefix_and_target() {
    let fixture = Fixture::ready().await;
    let mut attempt = fixture.attempt(&["first", "second"]);
    let first = revision(&attempt, 0);
    assert_eq!(
        attempt
            .await_artifact(0, ready(Ok(first.clone())))
            .await
            .unwrap(),
        SessionDraftPublicationProgress::ArtifactConfirmed { index: 0 }
    );
    let expected = attempt.allocated_artifacts()[1].clone();
    let mut request = Box::pin(attempt.await_artifact(1, pending()));
    poll_fn(|cx| {
        assert!(request.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(request);
    assert_eq!(
        attempt.state(),
        &SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Artifact {
            index: 1,
            reference: expected
        })
    );
    assert_eq!(attempt.confirmed_artifacts(), &[first]);
    assert!(attempt.confirmed_task().is_none());
    assert_eq!(
        attempt.advance(&fixture.token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
}

#[tokio::test]
async fn caller_timeout_keeps_the_final_task_unconfirmed_without_erasing_artifact_receipts() {
    let fixture = Fixture::ready().await;
    let mut attempt = fixture.attempt(&["text"]);
    let first = revision(&attempt, 0);
    attempt
        .await_artifact(0, ready(Ok(first.clone())))
        .await
        .unwrap();
    let expected = attempt.publication_manifest().unwrap();
    assert!(tokio::time::timeout(
        std::time::Duration::from_millis(1),
        attempt.await_task(expected, pending())
    )
    .await
    .is_err());
    assert_eq!(
        attempt.state(),
        &SessionDraftPublicationState::Unconfirmed(SessionDraftPublicationStep::Task {
            reference: attempt.task_ref()
        })
    );
    assert_eq!(attempt.confirmed_artifacts(), &[first]);
    assert!(attempt.confirmed_task().is_none());
    assert_eq!(
        attempt.advance(&fixture.token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
}
