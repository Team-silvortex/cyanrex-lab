use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_parsed_checkpoint_survives_attempt_drop_and_source_reopen() {
    for task in [false, true] {
        let f = Fixture::ready().await;
        let (attempt, checkpoint) = visible(&f, task).await;
        let fresh = fresh_same_owner(&f).await;
        let saved = checkpoint.to_json().unwrap();
        drop(attempt);
        drop(checkpoint);
        let checkpoint = SessionDraftPublicationCheckpoint::parse_json(&saved).unwrap();
        // Rebuild source and workspace handles; this is not a process-crash durability claim.
        let source = f.auth.base.reopen().await;
        let artifacts = source
            .open_artifact_workspace(&f.artifact_schema, scope(), &f.directory)
            .unwrap();
        let workspace =
            SessionTaskContentWorkspace::new(&f.task_schema, scope(), artifacts).unwrap();
        let before = snapshot(&f).await;
        let result = source
            .inspect_task_draft_checkpoint(&fresh, &workspace, &checkpoint)
            .await;
        let after = snapshot(&f).await;
        let expected = report(
            &checkpoint,
            SessionDraftCheckpointObservedOutcome::MatchesCheckpointMetadata,
        );
        let unchanged = checkpoint.to_json().unwrap();
        drop(workspace);
        drop(source);
        f.cleanup().await;
        assert_eq!(result, expected);
        assert_eq!(saved, unchanged);
        assert_eq!(before, after);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_fresh_session_is_independent_of_original_attempt_fingerprint() {
    let f = Fixture::ready().await;
    let (mut attempt, checkpoint) = visible(&f, false).await;
    let fresh = fresh_same_owner(&f).await;
    let state = attempt.state().clone();
    let before = snapshot(&f).await;
    let original_api = attempt.observe_unconfirmed(&fresh).await;
    let independent = inspect(&f, &fresh, &checkpoint).await;
    let stopped = attempt.advance(&f.auth.learner_token).await;
    let after = snapshot(&f).await;
    let expected = report(
        &checkpoint,
        SessionDraftCheckpointObservedOutcome::MatchesCheckpointMetadata,
    );
    let unchanged = attempt.state().clone();
    let empty = attempt.confirmed_artifacts().is_empty() && attempt.confirmed_task().is_none();
    f.cleanup().await;
    assert_eq!(
        original_api,
        Err(SessionDraftPublicationError::SessionChanged)
    );
    assert_eq!(independent, expected);
    assert_eq!(stopped, Err(SessionDraftPublicationError::Stopped));
    assert_eq!(state, unchanged);
    assert!(empty);
    assert_eq!(before, after);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_foreign_owner_cannot_read_the_checkpoint_target() {
    for task in [false, true] {
        let f = Fixture::ready().await;
        let (attempt, checkpoint) = visible(&f, task).await;
        drop(attempt);
        let before = snapshot(&f).await;
        // An active manager is authorized for private work, but not another owner's records.
        let manager = inspect(&f, &f.auth.manager_token, &checkpoint).await;
        let unbound = inspect(&f, &f.auth.target_token, &checkpoint).await;
        let after = snapshot(&f).await;
        let expected = report(
            &checkpoint,
            SessionDraftCheckpointObservedOutcome::NotVisible,
        );
        f.cleanup().await;
        let denied = SessionCommandError::Identity(IdentityStoreError::AccessDenied);
        assert_eq!(manager, expected);
        assert_eq!(
            unbound,
            Err(if task {
                SessionDraftCheckpointInspectionError::Task(SessionTaskError::Session(denied))
            } else {
                SessionDraftCheckpointInspectionError::Artifact(SessionArtifactError::Session(
                    denied,
                ))
            })
        );
        assert_eq!(before, after);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_metadata_and_labels_compare_only_observable_facts() {
    for task in [false, true] {
        let f = Fixture::ready().await;
        let (_, checkpoint) = visible(&f, task).await;
        let labels = changed(
            &checkpoint,
            "/manifest/payload/0/filename",
            json!("invented.txt"),
        );
        let labels = changed(
            &labels,
            "/manifest/payload/0/language",
            json!("future.other"),
        );
        let title = changed(
            &checkpoint,
            "/manifest/title",
            json!("A different reported title"),
        );
        let length = changed(&checkpoint, "/byte_lengths/0", json!(0));
        let before = snapshot(&f).await;
        let label_result = inspect(&f, &f.auth.learner_token, &labels).await;
        let title_result = inspect(&f, &f.auth.learner_token, &title).await;
        let length_result = inspect(&f, &f.auth.learner_token, &length).await;
        let after = snapshot(&f).await;
        let expected_labels = report(
            &labels,
            if task {
                SessionDraftCheckpointObservedOutcome::DiffersFromCheckpointMetadata
            } else {
                SessionDraftCheckpointObservedOutcome::MatchesCheckpointMetadata
            },
        );
        let expected_title = report(
            &title,
            SessionDraftCheckpointObservedOutcome::DiffersFromCheckpointMetadata,
        );
        let expected_length = report(
            &length,
            SessionDraftCheckpointObservedOutcome::DiffersFromCheckpointMetadata,
        );
        f.cleanup().await;
        assert_eq!(label_result, expected_labels); // Artifact stores have no filename/language facts.
        assert_eq!(title_result, expected_title);
        assert_eq!(length_result, expected_length);
        assert_eq!(before, after);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_exact_historical_artifact_is_not_a_current_task_receipt() {
    let f = Fixture::ready().await;
    let (artifact_attempt, artifact_checkpoint) = visible(&f, false).await;
    let original = artifact_attempt.allocated_artifacts()[0].clone();
    let draft = ArtifactDraft::new(
        "cyanrex.task-text".parse().unwrap(),
        TITLE,
        "text/plain",
        b"later bytes".to_vec(),
    )
    .unwrap();
    let revised = f
        .auth
        .source
        .revise_session_artifact(
            &f.auth.learner_token,
            &f.artifact_workspace,
            &original,
            id(),
            draft,
        )
        .await
        .unwrap();
    let (task_attempt, task_checkpoint) = visible(&f, true).await;
    let task = f
        .get(&f.auth.learner_token, task_attempt.task_ref().task_id)
        .await
        .unwrap()
        .unwrap()
        .snapshot;
    f.transition(&f.auth.learner_token, &task, TaskStatus::Ready)
        .await
        .unwrap();
    let before = snapshot(&f).await;
    let historical = inspect(&f, &f.auth.learner_token, &artifact_checkpoint).await;
    let current_task = inspect(&f, &f.auth.learner_token, &task_checkpoint).await;
    let after = snapshot(&f).await;
    let expected_artifact = report(
        &artifact_checkpoint,
        SessionDraftCheckpointObservedOutcome::MatchesCheckpointMetadata,
    );
    let expected_task = report(
        &task_checkpoint,
        SessionDraftCheckpointObservedOutcome::DiffersFromCheckpointMetadata,
    );
    f.cleanup().await;
    assert_ne!(revised.reference, original);
    assert_eq!(historical, expected_artifact);
    assert_eq!(current_task, expected_task);
    assert_eq!(before, after);
}
