use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_deferred_artifact_failure_is_not_visible_and_keeps_orphan_blob()
{
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, ITEMS);
    f.sql_artifact("CREATE SEQUENCE draft_fault_calls").await;
    f.sql_artifact(&format!(
        "CREATE FUNCTION draft_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
         PERFORM nextval('{}.draft_fault_calls'); RAISE EXCEPTION 'synthetic deferred failure'; END $$",
        f.artifact_schema,
    )).await;
    f.sql_artifact(
        "CREATE CONSTRAINT TRIGGER draft_fault AFTER INSERT ON collaboration_artifact_outbox
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION draft_fault()",
    )
    .await;
    let failed = attempt.advance(&f.auth.learner_token).await;
    f.auth.base.drain().await;
    let hit = artifact_fault_calls(&f).await;
    let before = snapshot(&f).await;
    let observed = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    let expected = report(&attempt, SessionDraftObservedOutcome::NotVisible);
    let stopped = attempt.advance(&f.auth.learner_token).await;
    let after = snapshot(&f).await;
    let remaining = counts(&f).await;
    let no_receipts =
        attempt.confirmed_artifacts().is_empty() && attempt.confirmed_task().is_none();
    f.cleanup().await;
    assert_eq!(
        failed,
        Err(SessionDraftPublicationError::Artifact(
            SessionArtifactError::Session(SessionCommandError::Authentication(
                DurableAuthError::StorageUnavailable
            ),)
        ))
    );
    assert_eq!(hit, (true, 1));
    assert_eq!(observed, expected);
    assert_eq!(before, after);
    assert_eq!(remaining, [0, 0, 0, 0, 0, 1]);
    assert!(no_receipts);
    assert_eq!(stopped, Err(SessionDraftPublicationError::Stopped));
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_missing_task_skips_artifact_namespace_and_later_match_does_not_resume(
) {
    let f = Fixture::ready().await;
    let mut attempt = unknown_task(&f, &[]).await;
    let state = attempt.state().clone();
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
        .await;
    let before = snapshot(&f).await;
    let missing = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    let after = snapshot(&f).await;
    let expected_missing = report(&attempt, SessionDraftObservedOutcome::NotVisible);
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 1")
        .await;
    // An independent, explicit command later creates a match. Observation must not adopt it.
    create_target(&f, &attempt, &[]).await;
    let created_before = snapshot(&f).await;
    let matching = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    let created_after = snapshot(&f).await;
    let expected_match = report(&attempt, SessionDraftObservedOutcome::MatchesPlannedCreate);
    // Unlike a missing Task, even an existing empty Task validates its Artifact namespace.
    f.sql_artifact("UPDATE collaboration_artifact_schema SET version = 999")
        .await;
    let invalid_before = snapshot(&f).await;
    let invalid_existing = attempt.observe_unconfirmed(&f.auth.learner_token).await;
    let invalid_after = snapshot(&f).await;
    let stopped = attempt.advance(&f.auth.learner_token).await;
    let final_state = attempt.state().clone();
    let no_task = attempt.confirmed_task().is_none();
    f.cleanup().await;
    assert_eq!(missing, expected_missing); // This read only validates source + Task namespaces.
    assert_eq!(matching, expected_match);
    assert_eq!(before, after);
    assert_eq!(created_before, created_after);
    assert_eq!(
        invalid_existing,
        Err(SessionDraftPublicationError::Task(
            SessionTaskError::Artifact(ArtifactStoreError::UnsupportedSchema,)
        ))
    );
    assert_eq!(invalid_before, invalid_after);
    assert_eq!(state, final_state);
    assert!(no_task);
    assert_eq!(stopped, Err(SessionDraftPublicationError::Stopped));
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_observation_artifact_and_task_blob_damage_remain_typed_errors() {
    for task in [false, true] {
        for missing in [false, true] {
            let f = Fixture::ready().await;
            let attempt = if task {
                let attempt = unknown_task(&f, ITEMS).await;
                create_target(&f, &attempt, ITEMS).await;
                attempt
            } else {
                let attempt = unknown_artifact(&f, ITEMS, 0).await;
                publish_target(&f, &attempt, ITEMS, 0).await;
                attempt
            };
            damage(&f, &attempt.allocated_artifacts()[0], missing);
            let state = attempt.state().clone();
            let before = snapshot(&f).await;
            let observed = attempt.observe_unconfirmed(&f.auth.learner_token).await;
            let after = snapshot(&f).await;
            let final_state = attempt.state().clone();
            f.cleanup().await;
            let expected = if task {
                SessionDraftPublicationError::Task(SessionTaskError::Artifact(
                    ArtifactStoreError::BlobUnavailable,
                ))
            } else {
                SessionDraftPublicationError::Artifact(SessionArtifactError::Artifact(
                    ArtifactStoreError::BlobUnavailable,
                ))
            };
            assert_eq!(observed, Err(expected));
            assert_eq!(before, after);
            assert_eq!(state, final_state);
        }
    }
}
