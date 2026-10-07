use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_binary_and_control_artifacts_differ_from_text_checkpoint() {
    for bytes in [vec![0xff, 0xfe], b"text\0control".to_vec()] {
        let f = Fixture::ready().await;
        let attempt = unknown_artifact(&f, ITEMS, 0).await;
        let allocated = &attempt.allocated_artifacts()[0];
        let created = f
            .auth
            .source
            .create_session_artifact(
                &f.auth.learner_token,
                &f.artifact_workspace,
                allocated.artifact_id,
                allocated.revision_id,
                ArtifactDraft::new(
                    "cyanrex.task-text".parse().unwrap(),
                    TITLE,
                    "text/plain",
                    bytes.clone(),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        // The unsigned descriptor can name real generic bytes, but that does not make them text.
        let checkpoint = changed(
            &checkpoint(&attempt),
            "/manifest/payload/0/artifact/sha256",
            json!(created.reference.sha256.as_str()),
        );
        let checkpoint = changed(&checkpoint, "/byte_lengths/0", json!(bytes.len()));
        let readable = f
            .auth
            .source
            .read_session_artifact(
                &f.auth.learner_token,
                &f.artifact_workspace,
                &created.reference,
            )
            .await
            .unwrap()
            .unwrap();
        let before = snapshot(&f).await;
        let result = inspect(&f, &f.auth.learner_token, &checkpoint).await;
        let after = snapshot(&f).await;
        let expected = report(
            &checkpoint,
            SessionDraftCheckpointObservedOutcome::DiffersFromCheckpointMetadata,
        );
        f.cleanup().await;
        assert_eq!(readable.bytes, bytes); // The underlying generic record really is valid/readable.
        assert_eq!(result, expected);
        assert_eq!(before, after);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_digest_blob_and_namespace_failures_are_typed() {
    for task in [false, true] {
        for fault in ["digest", "missing", "corrupt", "namespace"] {
            let f = Fixture::ready().await;
            let (attempt, original_checkpoint) = visible(&f, task).await;
            let checkpoint = if fault == "digest" {
                changed(
                    &original_checkpoint,
                    "/manifest/payload/0/artifact/sha256",
                    json!("b".repeat(64)),
                )
            } else {
                original_checkpoint
            };
            match fault {
                "missing" => damage(&f, &attempt.allocated_artifacts()[0], true),
                "corrupt" => damage(&f, &attempt.allocated_artifacts()[0], false),
                "namespace" => {
                    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
                        .await
                }
                _ => {}
            }
            let before = snapshot(&f).await;
            let result = inspect(&f, &f.auth.learner_token, &checkpoint).await;
            let after = snapshot(&f).await;
            let differs = report(
                &checkpoint,
                SessionDraftCheckpointObservedOutcome::DiffersFromCheckpointMetadata,
            );
            f.cleanup().await;
            if task && fault == "digest" {
                // The current Task is valid but differs from this altered descriptor; no forged
                // Artifact pin was used by C2-M's authorized read of the saved Task.
                assert_eq!(result, differs);
            } else {
                let error = match fault {
                    "digest" => ArtifactStoreError::InvalidRecord,
                    "namespace" => ArtifactStoreError::UnsupportedSchema,
                    _ => ArtifactStoreError::BlobUnavailable,
                };
                assert_eq!(
                    result,
                    Err(if task {
                        SessionDraftCheckpointInspectionError::Task(SessionTaskError::Artifact(
                            error,
                        ))
                    } else {
                        SessionDraftCheckpointInspectionError::Artifact(
                            SessionArtifactError::Artifact(error),
                        )
                    })
                );
            }
            assert_eq!(before, after);
        }
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_inspection_missing_task_skips_artifact_namespace_but_existing_empty_task_checks_it(
) {
    let f = Fixture::ready().await;
    let attempt = unknown_task(&f, &[]).await;
    let checkpoint = checkpoint(&attempt);
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
        .await;
    let before = snapshot(&f).await;
    let missing = inspect(&f, &f.auth.learner_token, &checkpoint).await;
    let after = snapshot(&f).await;
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 1")
        .await;
    create_target(&f, &attempt, &[]).await;
    let created_before = snapshot(&f).await;
    let matching = inspect(&f, &f.auth.learner_token, &checkpoint).await;
    let created_after = snapshot(&f).await;
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
        .await;
    let invalid_before = snapshot(&f).await;
    let invalid_existing = inspect(&f, &f.auth.learner_token, &checkpoint).await;
    let invalid_after = snapshot(&f).await;
    let expected_missing = report(
        &checkpoint,
        SessionDraftCheckpointObservedOutcome::NotVisible,
    );
    let expected_match = report(
        &checkpoint,
        SessionDraftCheckpointObservedOutcome::MatchesCheckpointMetadata,
    );
    f.cleanup().await;
    assert_eq!(missing, expected_missing);
    assert_eq!(matching, expected_match);
    assert_eq!(
        invalid_existing,
        Err(SessionDraftCheckpointInspectionError::Task(
            SessionTaskError::Artifact(ArtifactStoreError::UnsupportedSchema)
        ))
    );
    assert_eq!(before, after);
    assert_eq!(created_before, created_after);
    assert_eq!(invalid_before, invalid_after);
}
