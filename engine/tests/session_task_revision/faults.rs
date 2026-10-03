use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_missing_or_corrupt_old_and_new_blobs_cannot_be_dropped_away(
) {
    for (which, missing) in [("old", true), ("old", false), ("new", true), ("new", false)] {
        let (f, task, old, new) = pair().await;
        let reference = if which == "old" {
            &old.reference
        } else {
            &new.reference
        };
        let path = f.blob(reference);
        let mut damaged = fs::read(&path).unwrap();
        damaged[0] ^= 1;
        if missing {
            fs::remove_file(&path).unwrap();
        } else {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            fs::write(&path, &damaged).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        }
        assert_eq!(
            replace(
                &f,
                &f.auth.learner_token,
                &task,
                vec![new.reference.clone()]
            )
            .await,
            Err(SessionTaskError::Artifact(
                ArtifactStoreError::BlobUnavailable
            )),
            "{which}, missing={missing}"
        );
        unchanged(&f, &task, 1).await;
        assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 2);
        assert_eq!(f.files(), if missing { 1 } else { 2 });
        if missing {
            assert!(!path.exists());
        } else {
            assert_eq!(fs::read(&path).unwrap(), damaged);
        }
        let untouched = if which == "old" { &new } else { &old };
        assert!(f
            .artifacts
            .read(&untouched.reference, untouched.owner)
            .await
            .unwrap()
            .is_some());
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_event_failure_rolls_back_but_keeps_published_new_artifact()
{
    for deferred in [false, true] {
        let (f, task, old, new) = pair().await;
        f.sql_task("CREATE SEQUENCE replacement_fault_calls").await;
        f.sql_task(&format!("CREATE FUNCTION reject_replacement() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.replacement_fault_calls'); RAISE EXCEPTION 'synthetic replacement fault'; END $$", f.task_schema)).await;
        if deferred {
            f.sql_task("CREATE CONSTRAINT TRIGGER reject_replacement AFTER INSERT ON collaboration_task_outbox
                DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_replacement()").await;
        } else {
            f.sql_task(
                "CREATE TRIGGER reject_replacement BEFORE INSERT ON collaboration_task_outbox
                FOR EACH ROW EXECUTE FUNCTION reject_replacement()",
            )
            .await;
        }
        assert!(replace(
            &f,
            &f.auth.learner_token,
            &task,
            vec![new.reference.clone()]
        )
        .await
        .is_err());
        fault_reached(&f).await;
        unchanged(&f, &task, 1).await;
        published(&f, &old, &new).await;
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_rechecks_removed_and_added_artifact_records_after_task_event(
) {
    for which in ["old", "new", "schema"] {
        let (f, task, old, new) = pair().await;
        let body = if which == "schema" {
            format!(
                "UPDATE {}.collaboration_artifact_schema SET workspace_id = '{}';",
                f.artifact_schema,
                Uuid::new_v4()
            )
        } else {
            let reference = if which == "old" {
                &old.reference
            } else {
                &new.reference
            };
            format!("UPDATE {}.collaboration_artifact_revisions SET snapshot = '{{}}' WHERE revision_id = '{}';", f.artifact_schema, reference.revision_id)
        };
        fault_trigger(&f, &body).await;
        assert!(
            matches!(
                replace(
                    &f,
                    &f.auth.learner_token,
                    &task,
                    vec![new.reference.clone()]
                )
                .await,
                Err(SessionTaskError::Artifact(
                    ArtifactStoreError::InvalidRecord | ArtifactStoreError::ScopeMismatch
                ))
            ),
            "{which}"
        );
        fault_reached(&f).await;
        unchanged(&f, &task, 1).await;
        published(&f, &old, &new).await;
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_revision_task_tamper_and_session_deletion_cannot_escape_final_checks(
) {
    for target in ["task", "session"] {
        let (f, task, old, new) = pair().await;
        let session = f
            .auth
            .source
            .validate_session(&f.auth.learner_token)
            .await
            .unwrap()
            .unwrap();
        let body = if target == "session" {
            format!(
                "DELETE FROM {}.sessions WHERE token = '{}';",
                f.auth.base.schema,
                source_fixture::token_hash(&f.auth.learner_token)
            )
        } else {
            // Change both copies coherently; merely comparing head to latest event is insufficient.
            "NEW.snapshot := jsonb_set(NEW.snapshot::jsonb, '{title}', '\"Injected after validation\"')::text;
            UPDATE collaboration_tasks SET snapshot = NEW.snapshot WHERE task_id = NEW.task_id;".to_string()
        };
        fault_trigger(&f, &body).await;
        let expected = if target == "session" {
            SessionTaskError::Session(SessionCommandError::InvalidSession)
        } else {
            SessionTaskError::Task(TaskStoreError::InvalidRecord)
        };
        assert_eq!(
            replace(
                &f,
                &f.auth.learner_token,
                &task,
                vec![new.reference.clone()]
            )
            .await,
            Err(expected)
        );
        fault_reached(&f).await;
        unchanged(&f, &task, 1).await;
        assert_eq!(
            f.auth
                .source
                .validate_session(&f.auth.learner_token)
                .await
                .unwrap(),
            Some(session)
        );
        published(&f, &old, &new).await;
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}
