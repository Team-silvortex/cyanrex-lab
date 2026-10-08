use super::*;
use std::os::unix::fs::PermissionsExt;
fn journal(error: DraftIntentJournalError) -> SessionDraftJournalObservationError {
    SessionDraftJournalObservationError::Intent(SessionDraftIntentError::Journal(error))
}
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_rejects_schema_installation_and_routing_damage() {
    let old = intent_fixture::Fixture::ready().await;
    let attempt = prepare(&old, &[]);
    attempt
        .register_intent(&old.auth.learner_token, &old.intent_workspace)
        .await
        .unwrap();
    let error = old
        .auth
        .source
        .observe_session_draft_journal_step(
            &old.auth.learner_token,
            &old.intent_workspace,
            attempt.task_ref().task_id,
            0,
        )
        .await
        .err();
    old.cleanup().await;
    assert_eq!(
        error,
        Some(journal(DraftIntentJournalError::UnsupportedSchema))
    );
    for mode in [
        "version",
        "installation",
        "source_namespace",
        "task_namespace",
        "artifact_namespace",
    ] {
        let f = Fixture::ready().await;
        let attempt = complete(&f, &[]).await;
        let expected = match mode {
            "version" => {
                f.sql("UPDATE collaboration_draft_intent_schema SET version = 999")
                    .await;
                DraftIntentJournalError::UnsupportedSchema
            }
            "installation" => {
                f.sql(&format!(
                    "UPDATE collaboration_draft_intent_schema SET installation_id = '{}'",
                    Uuid::new_v4()
                ))
                .await;
                DraftIntentJournalError::IncarnationMismatch
            }
            _ => {
                f.sql(&format!(
                    "UPDATE collaboration_draft_intents SET {mode} = 'changed_namespace'"
                ))
                .await;
                DraftIntentJournalError::InvalidRecord
            }
        };
        let before = snapshot(&f).await;
        let result = observe(&f, &f.auth.learner_token, attempt.task_ref().task_id, 0).await;
        let intact = before == snapshot(&f).await;
        f.cleanup().await;
        assert_eq!(result, Err(journal(expected)), "{mode}");
        assert!(intact);
    }
}
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_rejects_corrupt_intents_and_step_sequences() {
    for mode in ["noncanonical", "checkpoint", "gap", "nonce", "false_prefix"] {
        let f = Fixture::ready().await;
        let attempt = complete(&f, ITEMS).await;
        match mode {
            "noncanonical" => f.sql("UPDATE collaboration_draft_intents SET checkpoint = convert_to(' ', 'UTF8') || checkpoint").await,
            "checkpoint" => f.sql("UPDATE collaboration_draft_intents SET checkpoint = convert_to('{}', 'UTF8')").await,
            "gap" => f.sql("DELETE FROM collaboration_draft_dispatch_steps WHERE ordinal = 1").await,
            "nonce" => f.sql("UPDATE collaboration_draft_dispatch_steps SET nonce = '00000000-0000-0000-0000-000000000000' WHERE ordinal = 0").await,
            _ => f.sql("UPDATE collaboration_draft_dispatch_steps SET committed = FALSE WHERE ordinal = 0").await,
        }
        let before = snapshot(&f).await;
        let result = observe(&f, &f.auth.learner_token, attempt.task_ref().task_id, 0).await;
        let intact = before == snapshot(&f).await;
        f.cleanup().await;
        assert_eq!(
            result,
            Err(journal(DraftIntentJournalError::InvalidRecord)),
            "{mode}"
        );
        assert!(intact);
    }
}
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_reads_exact_old_revision_and_distinguishes_metadata() {
    let f = Fixture::ready().await;
    let mut attempt = f.journaled(ITEMS).await;
    attempt.advance(&f.auth.learner_token).await.unwrap();
    let reference = attempt.allocated_artifacts()[0].clone();
    let newer = f
        .auth
        .source
        .revise_session_artifact(
            &f.auth.learner_token,
            &f.artifact_workspace,
            &reference,
            id(),
            ArtifactDraft::new(
                "cyanrex.task-text".parse().unwrap(),
                "New head",
                "text/plain",
                b"new bytes".to_vec(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let before = snapshot(&f).await;
    let old = observe(&f, &f.auth.learner_token, attempt.task_ref().task_id, 0).await;
    let old_intact = before == snapshot(&f).await;
    let other = uncertain(&f, false, ITEMS).await;
    let target = &other.allocated_artifacts()[0];
    f.auth
        .source
        .create_session_artifact(
            &f.auth.learner_token,
            &f.artifact_workspace,
            target.artifact_id,
            target.revision_id,
            ArtifactDraft::new(
                "cyanrex.task-text".parse().unwrap(),
                "Other valid title",
                "text/plain",
                ITEMS[0].2.as_bytes().to_vec(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let before = snapshot(&f).await;
    let differing = observe(&f, &f.auth.learner_token, other.task_ref().task_id, 0).await;
    let differing_intact = before == snapshot(&f).await;
    f.cleanup().await;
    assert_ne!(newer.reference.revision_id, reference.revision_id);
    let old = old.unwrap().unwrap();
    let differing = differing.unwrap().unwrap();
    assert_eq!(
        old.0,
        SessionDraftPublicationStep::Artifact {
            index: 0,
            reference
        }
    );
    assert_eq!(
        old.2,
        SessionDraftJournalObservedOutcome::MatchesIntentMetadata
    );
    assert_eq!(
        differing.2,
        SessionDraftJournalObservedOutcome::DiffersFromIntentMetadata
    );
    assert!(old_intact && differing_intact);
}
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_preserves_typed_blob_and_permission_errors() {
    for task in [false, true] {
        for mode in ["missing", "bytes", "permissions"] {
            let f = Fixture::ready().await;
            let attempt = complete(&f, ITEMS).await;
            let path = f.blob(&attempt.allocated_artifacts()[0]);
            match mode {
                "missing" => fs::remove_file(&path).unwrap(),
                "permissions" => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap()
                }
                _ => {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
                    fs::write(&path, b"synthetic broken bytes").unwrap();
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
                }
            }
            let before = snapshot(&f).await;
            let result = observe(
                &f,
                &f.auth.learner_token,
                attempt.task_ref().task_id,
                if task { ITEMS.len() } else { 0 },
            )
            .await;
            let intact = before == snapshot(&f).await;
            f.cleanup().await;
            assert_eq!(
                result,
                Err(if task {
                    SessionDraftJournalObservationError::Task(SessionTaskError::Artifact(
                        ArtifactStoreError::BlobUnavailable,
                    ))
                } else {
                    SessionDraftJournalObservationError::Artifact(SessionArtifactError::Artifact(
                        ArtifactStoreError::BlobUnavailable,
                    ))
                }),
                "{mode}, task={task}"
            );
            assert!(intact);
        }
    }
}
#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_journal_observation_missing_task_does_not_probe_artifact_namespace() {
    let f = Fixture::ready().await;
    let attempt = uncertain(&f, true, &[]).await;
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
        .await;
    let before = snapshot(&f).await;
    let missing = observe(&f, &f.auth.learner_token, attempt.task_ref().task_id, 0).await;
    let missing_intact = before == snapshot(&f).await;
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 1")
        .await;
    create_task(&f, &attempt).await;
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
        .await;
    let before = snapshot(&f).await;
    let existing = observe(&f, &f.auth.learner_token, attempt.task_ref().task_id, 0).await;
    let existing_intact = before == snapshot(&f).await;
    f.cleanup().await;
    assert_eq!(
        missing.unwrap().unwrap().2,
        SessionDraftJournalObservedOutcome::NotVisible
    );
    assert_eq!(
        existing,
        Err(SessionDraftJournalObservationError::Task(
            SessionTaskError::Artifact(ArtifactStoreError::UnsupportedSchema)
        ))
    );
    assert!(missing_intact && existing_intact);
}
