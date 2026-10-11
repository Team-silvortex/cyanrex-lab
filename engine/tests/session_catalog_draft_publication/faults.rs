use super::sql_fixture::*;
use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_publication_second_artifact_fault_preserves_prefix_orphan_and_terminal_target(
) {
    let f = Fixture::ready().await;
    let mut attempt = prepare(
        &f,
        &values()[1],
        &[
            ("a", "text", "one"),
            ("b", "text", "two"),
            ("c", "text", "three"),
        ],
    );
    attempt.advance(&f.auth.learner_token).await.unwrap();
    let first = attempt.confirmed_artifacts().to_vec();
    let target = attempt.allocated_artifacts()[1].clone();
    f.sql_artifact("CREATE SEQUENCE draft_fault_calls").await;
    f.sql_artifact(&format!("CREATE FUNCTION draft_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('{}.draft_fault_calls'); RETURN NULL; END $$", f.artifact_schema)).await;
    f.sql_artifact("CREATE TRIGGER draft_fault BEFORE INSERT ON collaboration_artifact_outbox FOR EACH ROW EXECUTE FUNCTION draft_fault()").await;
    let before = observed(&f).await;
    assert!(attempt.advance(&f.auth.learner_token).await.is_err());
    f.auth.base.drain().await;
    assert_eq!(artifact_fault_calls(&f).await, (true, 1));
    assert_eq!(attempt.state(), &uncertain_artifact(1, target.clone()));
    assert_eq!(attempt.confirmed_artifacts(), first);
    assert!(attempt.confirmed_task().is_none());
    assert_eq!(counts(&f).await, [1, 1, 1, 0, 0, 2]);
    assert_eq!(fs::read(f.blob(&first[0].reference)).unwrap(), b"one");
    assert_eq!(
        fs::read(f.blob(&target)).unwrap(),
        b"two",
        "unknown publication blob is not silently deleted"
    );
    let after = observed(&f).await;
    assert_eq!(
        after.rows, before.rows,
        "every committed resource row is preserved"
    );
    let mut expected_files = before.files;
    expected_files.push((
        f.blob(&target)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        b"two".to_vec(),
    ));
    expected_files.sort();
    assert_eq!(
        after.files, expected_files,
        "only the exact unconfirmed blob is added"
    );
    f.sql_artifact("DROP TRIGGER draft_fault ON collaboration_artifact_outbox")
        .await;
    let before = observed(&f).await;
    assert_eq!(
        attempt.advance(&f.auth.learner_token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
    assert_eq!(observed(&f).await, before);
    f.assert_pool_restored().await;
    drop(attempt);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_publication_final_task_definition_outbox_session_and_commit_faults_never_confirm_or_retry(
) {
    for mode in ["definition", "event", "session", "membership", "deferred"] {
        let f = Fixture::ready().await;
        let mut attempt = prepare(&f, &values()[0], &[("a", "text", "confirmed input")]);
        attempt.advance(&f.auth.learner_token).await.unwrap();
        let first = attempt.confirmed_artifacts().to_vec();
        let target = attempt.task_ref();
        if mode == "deferred" {
            f.sql_task("CREATE SEQUENCE content_fault_calls").await;
            f.sql_task(&format!("CREATE FUNCTION content_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('{}.content_fault_calls'); RAISE EXCEPTION 'private commit fault'; END $$", f.task_schema)).await;
            f.sql_task("CREATE CONSTRAINT TRIGGER content_fault AFTER INSERT ON collaboration_task_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION content_fault()").await;
        } else {
            let body = match mode {
                "definition" => "NEW.snapshot = jsonb_set(NEW.snapshot::jsonb, '{definition,summary}', '\"Changed\"')::text; UPDATE collaboration_tasks SET snapshot = NEW.snapshot WHERE task_id = NEW.task_id;".into(),
                "event" => "RETURN NULL;".into(),
                "session" => format!("DELETE FROM {}.sessions WHERE token = '{}';", f.auth.base.schema, source_fixture::token_hash(&f.auth.learner_token)),
                "membership" => format!("UPDATE {}.collaboration_memberships SET status = 'suspended' WHERE principal_id = '{}';", f.auth.base.schema, f.auth.learner.binding.principal_id),
                _ => unreachable!(),
            };
            f.fault_trigger(&body).await;
        }
        let before = observed(&f).await;
        assert!(
            attempt.advance(&f.auth.learner_token).await.is_err(),
            "mode {mode}"
        );
        assert_eq!(f.fault_calls().await, 1, "effective fault {mode}");
        assert_eq!(attempt.state(), &uncertain_task(target));
        assert_eq!(attempt.confirmed_artifacts(), first);
        assert!(attempt.confirmed_task().is_none());
        assert_eq!(
            observed(&f).await,
            before,
            "all resource rows/files preserved at {mode}"
        );
        f.sql_task("DROP TRIGGER content_fault ON collaboration_task_outbox")
            .await;
        assert_eq!(
            attempt.advance(&f.auth.learner_token).await,
            Err(SessionDraftPublicationError::Stopped)
        );
        assert_eq!(observed(&f).await, before);
        assert!(f
            .auth
            .source
            .read_session_artifact(
                &f.auth.learner_token,
                &f.artifact_workspace,
                &first[0].reference
            )
            .await
            .unwrap()
            .is_some());
        assert!(f
            .auth
            .source
            .get_session_catalog_content_task(&f.auth.learner_token, &workspace(&f), target.task_id)
            .await
            .unwrap()
            .is_none());
        f.assert_pool_restored().await;
        drop(attempt);
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_publication_damaged_confirmed_bytes_and_preallocated_conflicts_are_not_adopted(
) {
    for missing in [false, true] {
        let f = Fixture::ready().await;
        let mut attempt = prepare(&f, &values()[1], &[("a", "text", "confirmed input")]);
        attempt.advance(&f.auth.learner_token).await.unwrap();
        let first = attempt.confirmed_artifacts().to_vec();
        let path = f.blob(&first[0].reference);
        if missing {
            fs::remove_file(&path).unwrap();
        } else {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            fs::write(&path, b"Externally corrupt").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        }
        let before = observed(&f).await;
        assert!(attempt.advance(&f.auth.learner_token).await.is_err());
        assert_eq!(attempt.state(), &uncertain_task(attempt.task_ref()));
        assert_eq!(attempt.confirmed_artifacts(), first);
        assert!(attempt.confirmed_task().is_none());
        assert_eq!(observed(&f).await, before);
        assert_eq!(
            attempt.advance(&f.auth.learner_token).await,
            Err(SessionDraftPublicationError::Stopped)
        );
        assert_eq!(observed(&f).await, before);
        drop(attempt);
        f.cleanup().await;
    }
    let f = Fixture::ready().await;
    let items = [("a", "text", "same bytes")];
    let mut attempt = prepare(&f, &values()[1], &items);
    let target = attempt.allocated_artifacts()[0].clone();
    f.auth
        .source
        .create_session_artifact(
            &f.auth.learner_token,
            &f.artifact_workspace,
            target.artifact_id,
            target.revision_id,
            manual_fixture::plan(&items).publication_draft(0).unwrap(),
        )
        .await
        .unwrap();
    let before = observed(&f).await;
    assert_eq!(
        attempt.advance(&f.auth.learner_token).await,
        Err(SessionDraftPublicationError::Artifact(
            SessionArtifactError::Artifact(ArtifactStoreError::Conflict)
        ))
    );
    assert_eq!(attempt.state(), &uncertain_artifact(0, target));
    assert!(attempt.confirmed_artifacts().is_empty());
    assert_eq!(observed(&f).await, before);
    assert_eq!(
        attempt.advance(&f.auth.learner_token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
    assert_eq!(observed(&f).await, before);
    drop(attempt);
    f.cleanup().await;
    let f = Fixture::ready().await;
    let mut attempt = prepare(&f, &values()[0], &[]);
    let target = attempt.task_ref();
    let manifest = TaskContentManifest::new(manual_fixture::TITLE, Vec::new()).unwrap();
    let existing = f
        .auth
        .source
        .create_session_catalog_content_task(
            &f.auth.learner_token,
            &workspace(&f),
            target.task_id,
            &values()[0].reference,
            manifest,
        )
        .await
        .unwrap();
    assert_eq!(existing.task.definition, Some(values()[0].clone()));
    let before = observed(&f).await;
    assert_eq!(
        attempt.advance(&f.auth.learner_token).await,
        Err(SessionDraftPublicationError::Task(SessionTaskError::Task(
            TaskStoreError::Conflict
        )))
    );
    assert_eq!(attempt.state(), &uncertain_task(target));
    assert!(attempt.confirmed_artifacts().is_empty());
    assert!(
        attempt.confirmed_task().is_none(),
        "a matching existing Task is not adopted"
    );
    assert_eq!(observed(&f).await, before);
    assert_eq!(
        attempt.advance(&f.auth.learner_token).await,
        Err(SessionDraftPublicationError::Stopped)
    );
    assert_eq!(observed(&f).await, before);
    drop(attempt);
    f.cleanup().await;
}
