use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_second_artifact_failure_preserves_prefix_and_stops() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(
        &f,
        &[
            ("a", "text", "one"),
            ("b", "text", "two"),
            ("c", "text", "three"),
        ],
    );
    attempt.advance(&f.auth.learner_token).await.unwrap();
    let first = attempt.confirmed_artifacts().to_vec();
    let failed_ref = attempt.allocated_artifacts()[1].clone();
    f.sql_artifact("CREATE SEQUENCE draft_fault_calls").await;
    f.sql_artifact(&format!(
        "CREATE FUNCTION draft_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
         PERFORM nextval('{}.draft_fault_calls'); RETURN NULL; END $$",
        f.artifact_schema,
    ))
    .await;
    f.sql_artifact(
        "CREATE TRIGGER draft_fault BEFORE INSERT ON collaboration_artifact_outbox
        FOR EACH ROW EXECUTE FUNCTION draft_fault()",
    )
    .await;
    let result = attempt.advance(&f.auth.learner_token).await;
    f.auth.base.drain().await;
    let state = attempt.state().clone();
    let confirmed = attempt.confirmed_artifacts().to_vec();
    let hit = artifact_fault_calls(&f).await;
    let after = counts(&f).await;
    f.sql_artifact("DROP TRIGGER draft_fault ON collaboration_artifact_outbox")
        .await;
    let retry = attempt.advance(&f.auth.learner_token).await;
    let after_retry = counts(&f).await;
    let readable = f
        .artifacts
        .read(&first[0].reference, first[0].owner)
        .await
        .unwrap();
    f.cleanup().await;
    assert_eq!(
        result,
        Err(SessionDraftPublicationError::Artifact(
            SessionArtifactError::Artifact(ArtifactStoreError::StorageUnavailable,)
        ))
    );
    assert_eq!(state, uncertain_artifact(1, failed_ref));
    assert_eq!(hit, (true, 1));
    assert_eq!(confirmed, first);
    assert_eq!(readable.unwrap().bytes, b"one");
    assert_eq!(after, [1, 1, 1, 0, 0, 2]); // Failed command's blob is not silently deleted.
    assert_eq!(retry, Err(SessionDraftPublicationError::Stopped));
    assert_eq!(after_retry, after);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_final_commit_failure_never_confirms_task_or_retries() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, &[("a", "text", "committed input")]);
    attempt.advance(&f.auth.learner_token).await.unwrap();
    let artifacts = attempt.confirmed_artifacts().to_vec();
    let target = attempt.task_ref();
    f.sql_task("CREATE SEQUENCE content_fault_calls").await;
    f.sql_task(&format!(
        "CREATE FUNCTION content_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
         PERFORM nextval('{}.content_fault_calls'); RAISE EXCEPTION 'synthetic deferred fault'; END $$",
        f.task_schema,
    )).await;
    f.sql_task(
        "CREATE CONSTRAINT TRIGGER content_fault AFTER INSERT ON collaboration_task_outbox
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION content_fault()",
    )
    .await;
    let result = attempt.advance(&f.auth.learner_token).await;
    f.auth.base.drain().await;
    let hit = f.fault_calls().await;
    let state = attempt.state().clone();
    let confirmed = attempt.confirmed_artifacts().to_vec();
    let task = attempt.confirmed_task().cloned();
    let after = counts(&f).await;
    f.sql_task("DROP TRIGGER content_fault ON collaboration_task_outbox")
        .await;
    let retry = attempt.advance(&f.auth.learner_token).await;
    let after_retry = counts(&f).await;
    f.cleanup().await;
    assert_eq!(
        result,
        Err(SessionDraftPublicationError::Task(
            SessionTaskError::Session(SessionCommandError::Authentication(
                DurableAuthError::StorageUnavailable
            ),)
        ))
    );
    assert_eq!(hit, 1);
    assert_eq!(state, uncertain_task(target));
    assert_eq!(confirmed, artifacts);
    assert!(task.is_none());
    assert_eq!(after, [1, 1, 1, 0, 0, 1]);
    assert_eq!(retry, Err(SessionDraftPublicationError::Stopped));
    assert_eq!(after_retry, after);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_confirmed_blob_damage_prevents_task_without_erasing_receipt() {
    for missing in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = prepare(&f, &[("a", "text", "confirmed input")]);
        attempt.advance(&f.auth.learner_token).await.unwrap();
        let artifacts = attempt.confirmed_artifacts().to_vec();
        let target = attempt.task_ref();
        let path = f.blob(&artifacts[0].reference);
        if missing {
            fs::remove_file(&path).unwrap();
        } else {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            fs::write(&path, b"external synthetic damage").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        }
        let result = attempt.advance(&f.auth.learner_token).await;
        let state = attempt.state().clone();
        let confirmed = attempt.confirmed_artifacts().to_vec();
        let task = attempt.confirmed_task().cloned();
        let retry = attempt.advance(&f.auth.learner_token).await;
        let after = counts(&f).await;
        f.cleanup().await;
        assert_eq!(
            result,
            Err(SessionDraftPublicationError::Task(
                SessionTaskError::Artifact(ArtifactStoreError::BlobUnavailable,)
            ))
        );
        assert_eq!(state, uncertain_task(target));
        assert_eq!(confirmed, artifacts); // Historical confirmation is not a claim of current bytes.
        assert!(task.is_none());
        assert_eq!(retry, Err(SessionDraftPublicationError::Stopped));
        assert_eq!(after, [1, 1, 1, 0, 0, if missing { 0 } else { 1 }]);
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_draft_publication_existing_allocated_target_is_conflict_not_adoption() {
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, &[("a", "text", "same bytes")]);
    let reference = attempt.allocated_artifacts()[0].clone();
    let existing = f
        .auth
        .source
        .create_session_artifact(
            &f.auth.learner_token,
            &f.artifact_workspace,
            reference.artifact_id,
            reference.revision_id,
            plan(&[("a", "text", "same bytes")])
                .publication_draft(0)
                .unwrap(),
        )
        .await
        .unwrap();
    let before = counts(&f).await;
    let result = attempt.advance(&f.auth.learner_token).await;
    let state = attempt.state().clone();
    let empty = attempt.confirmed_artifacts().is_empty() && attempt.confirmed_task().is_none();
    let retry = attempt.advance(&f.auth.learner_token).await;
    let after = counts(&f).await;
    let read = f.artifacts.read(&reference, existing.owner).await.unwrap();
    f.cleanup().await;
    assert_eq!(existing.reference, reference);
    assert_eq!(
        result,
        Err(SessionDraftPublicationError::Artifact(
            SessionArtifactError::Artifact(ArtifactStoreError::Conflict,)
        ))
    );
    assert_eq!(state, uncertain_artifact(0, reference));
    assert!(empty);
    assert_eq!(retry, Err(SessionDraftPublicationError::Stopped));
    assert_eq!(before, [1, 1, 1, 0, 0, 1]);
    assert_eq!(after, before);
    assert_eq!(read.unwrap().revision, existing);
}
