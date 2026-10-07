use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_ready_unpolled_and_complete_have_no_unknown_step() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, &[]);
    drop(attempt.advance(&f.auth.learner_token));
    let before = snapshot(&f).await;
    let ready = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    let ready_wrong = attempt.observe_unconfirmed(&f.auth.manager_token).await;
    let after_ready = snapshot(&f).await;
    attempt.advance(&f.auth.learner_token).await.unwrap();
    let completed = attempt.confirmed_task().cloned();
    let complete_before = snapshot(&f).await;
    let complete = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    let complete_after = snapshot(&f).await;
    let state = attempt.state().clone();
    let unchanged = attempt.confirmed_task().cloned();
    f.cleanup().await;
    for result in [ready, ready_wrong, complete] {
        assert_eq!(result, Err(SessionDraftPublicationError::NoUnconfirmedStep));
    }
    assert_eq!(before, after_ready);
    assert_eq!(complete_before, complete_after);
    assert_eq!(completed, unchanged);
    assert_eq!(state, SessionDraftPublicationState::Complete);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_fresh_tokens_cannot_replace_original_even_for_same_owner() {
    let f = Fixture::ready().await;
    let attempt = unknown_artifact(&f, ITEMS, 0).await;
    let account = f
        .auth
        .source
        .validate_session(&f.auth.learner_token)
        .await
        .unwrap()
        .unwrap()
        .account;
    let fresh = f.auth.base.seed_session(&account).await;
    let before = snapshot(&f).await;
    // A source writer lock proves these preflight errors do not even enter authentication.
    let mut holder = f.auth.base.pool.begin().await.unwrap();
    query("SELECT singleton FROM collaboration_auth_source_schema FOR UPDATE")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let results = tokio::time::timeout(Duration::from_secs(1), async {
        let mut results = Vec::new();
        for token in [&fresh.token, &f.auth.manager_token, &"invalid".to_owned()] {
            results.push(attempt.observe_unconfirmed(token).await);
        }
        results
    })
    .await;
    holder.rollback().await.unwrap();
    let after = snapshot(&f).await;
    f.cleanup().await;
    let results =
        results.expect("different tokens must be rejected without waiting for source SQL");
    assert!(results
        .into_iter()
        .all(|result| result == Err(SessionDraftPublicationError::SessionChanged)));
    assert_eq!(before, after);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_matching_existing_artifact_is_not_a_recovered_receipt() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, ITEMS);
    // Separate publication occupies the allocated target. This is not a simulated lost ACK.
    publish_target(&f, &attempt, ITEMS, 0).await;
    let conflict = attempt.advance(&f.auth.learner_token).await;
    let expected = report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate);
    let state = attempt.state().clone();
    let before = snapshot(&f).await;
    let observed = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    let stopped = attempt.advance(&f.auth.learner_token).await;
    let after = snapshot(&f).await;
    let final_state = attempt.state().clone();
    let empty = attempt.confirmed_artifacts().is_empty() && attempt.confirmed_task().is_none();
    f.cleanup().await;
    assert_eq!(
        conflict,
        Err(SessionDraftPublicationError::Artifact(
            SessionArtifactError::Artifact(ArtifactStoreError::Conflict)
        ))
    );
    assert_eq!(observed, expected);
    assert_eq!(state, final_state);
    assert!(empty);
    assert_eq!(stopped, Err(SessionDraftPublicationError::Stopped));
    assert_eq!(before, after);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_task_match_and_later_edits_never_change_attempt_progress() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, ITEMS);
    attempt.advance(&f.auth.learner_token).await.unwrap();
    let task = create_target(&f, &attempt, ITEMS).await;
    let conflict = attempt.advance(&f.auth.learner_token).await;
    let state = attempt.state().clone();
    let receipts = attempt.confirmed_artifacts().to_vec();
    let before = snapshot(&f).await;
    let matching = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    let after = snapshot(&f).await;
    let manifest = TaskContentManifest::new(
        "Changed title",
        vec![TextPayloadBinding::new(
            attempt.allocated_artifacts()[0].clone(),
            "renamed.txt",
            "plaintext",
        )
        .unwrap()],
    )
    .unwrap();
    let edited = f
        .replace(&f.auth.learner_token, &task, manifest)
        .await
        .unwrap();
    let edit_before = snapshot(&f).await;
    let changed = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    let edit_after = snapshot(&f).await;
    f.transition(&f.auth.learner_token, &edited, TaskStatus::Ready)
        .await
        .unwrap();
    let status_before = snapshot(&f).await;
    let transitioned = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    let status_after = snapshot(&f).await;
    let expected_match = report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate);
    let expected_differs = report(
        &attempt,
        SessionDraftObservedOutcome::DiffersFromPlannedCreate,
    );
    let final_state = attempt.state().clone();
    let final_receipts = attempt.confirmed_artifacts().to_vec();
    let no_task = attempt.confirmed_task().is_none();
    let stopped = attempt.advance(&f.auth.learner_token).await;
    f.cleanup().await;
    assert_eq!(
        conflict,
        Err(SessionDraftPublicationError::Task(SessionTaskError::Task(
            TaskStoreError::Conflict
        )))
    );
    assert_eq!(matching, expected_match);
    assert_eq!(changed, expected_differs);
    assert_eq!(transitioned, expected_differs);
    assert_eq!(before, after);
    assert_eq!(edit_before, edit_after);
    assert_eq!(status_before, status_after);
    assert_eq!(state, final_state);
    assert_eq!(receipts, final_receipts);
    assert!(no_task);
    assert_eq!(stopped, Err(SessionDraftPublicationError::Stopped));
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_current_artifact_does_not_claim_to_revalidate_earlier_prefix() {
    let f = Fixture::ready().await;
    let items = [("one", "text", "same"), ("two", "text", "same")];
    let attempt = unknown_artifact(&f, &items, 1).await;
    publish_target(&f, &attempt, &items, 1).await;
    let receipts = attempt.confirmed_artifacts().to_vec();
    damage(&f, &receipts[0].reference, false);
    let before = snapshot(&f).await;
    let result = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    let after = snapshot(&f).await;
    let expected = report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate);
    let final_receipts = attempt.confirmed_artifacts().to_vec();
    f.cleanup().await;
    assert_eq!(result, expected);
    assert_eq!(receipts, final_receipts);
    assert_eq!(before, after);
}
