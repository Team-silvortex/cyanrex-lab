use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_empty_plan_creates_only_one_authorized_task() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, &[]);
    let target = attempt.task_ref();
    let before = counts(&f).await;
    let result = attempt.advance(&f.auth.learner_token).await;
    let state = attempt.state().clone();
    let confirmed = attempt.confirmed_task().cloned();
    let read = f.get(&f.auth.learner_token, target.task_id).await.unwrap();
    let stopped = attempt.advance(&f.auth.learner_token).await;
    let after = counts(&f).await;
    f.cleanup().await;
    assert_eq!(before, [0; 6]);
    assert_eq!(result, Ok(SessionDraftPublicationProgress::TaskConfirmed));
    assert_eq!(state, SessionDraftPublicationState::Complete);
    assert_eq!(stopped, Err(SessionDraftPublicationError::Stopped));
    assert_eq!(after, [0, 0, 0, 1, 1, 0]);
    let read = read.unwrap();
    assert_eq!(confirmed, Some(read.snapshot.clone()));
    assert_eq!(read.snapshot.task.reference, target);
    assert_eq!(read.snapshot.task.title, TITLE);
    assert_eq!(read.snapshot.task.status, TaskStatus::Draft);
    assert!(read.snapshot.manifest.payload().is_empty());
    assert!(read.contents.is_empty());
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_orders_unicode_and_same_bytes_using_distinct_server_refs() {
    let f = Fixture::ready().await;
    let items = [
        ("同名.txt", "future.language", "文档\r\n\t😀"),
        ("同名.txt", "plaintext", "文档\r\n\t😀"),
        ("empty.rs", "rust", ""),
    ];
    let mut attempt = prepare(&f, &items);
    let target = attempt.task_ref();
    let refs = attempt.allocated_artifacts().to_vec();
    let owner = f.auth.learner.principal.reference;
    let mut steps = Vec::new();
    for _ in 0..items.len() {
        let progress = attempt.advance(&f.auth.learner_token).await;
        steps.push((progress, attempt.state().clone(), counts(&f).await));
    }
    let artifact_receipts = attempt.confirmed_artifacts().to_vec();
    let final_result = attempt.advance(&f.auth.learner_token).await;
    let complete = attempt.confirmed_task().cloned();
    let read = f.get(&f.auth.learner_token, target.task_id).await.unwrap();
    let after = counts(&f).await;
    f.cleanup().await;
    assert_eq!(refs[0].sha256, refs[1].sha256);
    assert_ne!(refs[0].artifact_id, refs[1].artifact_id);
    assert_ne!(refs[0].revision_id, refs[1].revision_id);
    for (index, (progress, state, count)) in steps.into_iter().enumerate() {
        assert_eq!(
            progress,
            Ok(SessionDraftPublicationProgress::ArtifactConfirmed { index })
        );
        assert_eq!(state, SessionDraftPublicationState::Ready);
        let n = (index + 1) as i64;
        assert_eq!(count, [n, n, n, 0, 0, n]);
        assert_eq!(artifact_receipts[index].reference, refs[index]);
        assert_eq!(artifact_receipts[index].owner, owner);
        assert_eq!(artifact_receipts[index].sequence, 1.try_into().unwrap());
    }
    assert_eq!(
        final_result,
        Ok(SessionDraftPublicationProgress::TaskConfirmed)
    );
    assert_eq!(after, [3, 3, 3, 1, 1, 3]);
    let read = read.unwrap();
    assert_eq!(complete, Some(read.snapshot.clone()));
    assert_eq!(read.snapshot.task.owner, owner);
    assert_eq!(read.snapshot.task.input_refs, refs);
    for (index, (filename, language, text)) in items.iter().enumerate() {
        let binding = &read.snapshot.manifest.payload()[index];
        assert_eq!(binding.filename(), *filename);
        assert_eq!(binding.language(), *language);
        assert_eq!(binding.artifact(), &refs[index]);
        assert_eq!(read.contents[index].revision, artifact_receipts[index]);
        assert_eq!(read.contents[index].bytes, text.as_bytes());
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_unpolled_advance_preserves_ready_without_dispatch() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, &[("a.txt", "plaintext", "local bytes")]);
    let refs = attempt.allocated_artifacts().to_vec();
    drop(attempt.advance(&f.auth.learner_token));
    let state = attempt.state().clone();
    let before = counts(&f).await;
    let result = attempt.advance(&f.auth.learner_token).await;
    let confirmed = attempt.confirmed_artifacts().to_vec();
    let after = counts(&f).await;
    f.cleanup().await;
    assert_eq!(state, SessionDraftPublicationState::Ready);
    assert_eq!(before, [0; 6]);
    assert_eq!(
        result,
        Ok(SessionDraftPublicationProgress::ArtifactConfirmed { index: 0 })
    );
    assert_eq!(confirmed[0].reference, refs[0]);
    assert_eq!(after, [1, 1, 1, 0, 0, 1]);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_wrong_token_never_dispatches_and_original_can_continue() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, &[]);
    let changed = attempt.advance(&f.auth.manager_token).await;
    let invalid = attempt.advance("not-a-session").await;
    let state = attempt.state().clone();
    let before = counts(&f).await;
    let result = attempt.advance(&f.auth.learner_token).await;
    let owner = attempt.confirmed_task().map(|value| value.task.owner);
    let expected_owner = f.auth.learner.principal.reference;
    f.cleanup().await;
    assert_eq!(changed, Err(SessionDraftPublicationError::SessionChanged));
    assert_eq!(invalid, Err(SessionDraftPublicationError::SessionChanged));
    assert_eq!(state, SessionDraftPublicationState::Ready);
    assert_eq!(before, [0; 6]);
    assert_eq!(result, Ok(SessionDraftPublicationProgress::TaskConfirmed));
    assert_eq!(owner, Some(expected_owner));
}
