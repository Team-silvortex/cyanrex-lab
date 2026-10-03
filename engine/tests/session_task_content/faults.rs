use super::*;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_corrupt_old_bytes_cannot_be_removed_or_hidden_by_metadata_edits()
{
    for missing in [false, true] {
        let (f, task, old, new) = pair().await;
        let path = f.blob(&old.reference);
        if missing {
            fs::remove_file(&path).unwrap();
        } else {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            fs::write(&path, b"Corrupted data").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        }
        assert!(f
            .get(&f.auth.learner_token, task.task.reference.task_id)
            .await
            .is_err());
        for requested in [
            manifest("Metadata only", vec![old.reference.clone()]),
            manifest("Remove last item", vec![]),
            manifest("Replace item", vec![new.reference.clone()]),
        ] {
            assert_eq!(
                f.replace(&f.auth.learner_token, &task, requested).await,
                Err(SessionTaskError::Artifact(
                    ArtifactStoreError::BlobUnavailable
                ))
            );
        }
        assert_eq!(
            f.transition(&f.auth.learner_token, &task, TaskStatus::Cancelled)
                .await,
            Err(SessionTaskError::Artifact(
                ArtifactStoreError::BlobUnavailable
            ))
        );
        f.unchanged(&task, 1).await;
        assert_eq!(f.files(), if missing { 1 } else { 2 });
        assert!(f
            .artifacts
            .read(&new.reference, new.owner)
            .await
            .unwrap()
            .is_some());
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_trusted_nontext_old_content_is_not_reclassified_as_a_valid_edit()
{
    let f = Fixture::ready().await;
    let binary = f.create_artifact(&f.auth.learner_token, &[0xff]).await;
    // Trusted direct storage can pin arbitrary bytes; a later Session read/edit must revalidate them.
    let task = f
        .tasks
        .create(
            id(),
            f.auth.learner.principal.reference,
            manifest("Trusted but not text", vec![binary.reference]),
        )
        .await
        .unwrap();
    assert_eq!(
        f.get(&f.auth.learner_token, task.task.reference.task_id)
            .await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidRecord))
    );
    assert_eq!(
        f.replace(
            &f.auth.learner_token,
            &task,
            manifest("Cannot erase validation", vec![])
        )
        .await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidRecord))
    );
    assert_eq!(
        f.transition(&f.auth.learner_token, &task, TaskStatus::Cancelled)
            .await,
        Err(SessionTaskError::Task(TaskStoreError::InvalidRecord))
    );
    f.unchanged(&task, 1).await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_event_suppression_and_deferred_failure_roll_back_without_deleting_artifacts(
) {
    for deferred in [false, true] {
        let (f, task, _, new) = pair().await;
        if deferred {
            f.sql_task("CREATE SEQUENCE content_fault_calls").await;
            f.sql_task(&format!("CREATE FUNCTION content_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                PERFORM nextval('{}.content_fault_calls'); RAISE EXCEPTION 'synthetic commit fault'; END $$", f.task_schema)).await;
            f.sql_task(
                "CREATE CONSTRAINT TRIGGER content_fault AFTER INSERT ON collaboration_task_outbox
                DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION content_fault()",
            )
            .await;
        } else {
            f.fault_trigger("RETURN NULL;").await;
        }
        assert!(f
            .replace(
                &f.auth.learner_token,
                &task,
                manifest("Replacement", vec![new.reference.clone()])
            )
            .await
            .is_err());
        assert!(f
            .transition(&f.auth.learner_token, &task, TaskStatus::Ready)
            .await
            .is_err());
        assert!(f
            .request(
                &f.auth.learner_token,
                id(),
                manifest("Creation", vec![new.reference])
            )
            .await
            .is_err());
        assert_eq!(f.fault_calls().await, 3);
        f.unchanged(&task, 1).await;
        assert_eq!(f.count_task("collaboration_tasks").await, 1);
        assert_eq!(f.files(), 2);
        assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 2);
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_old_new_artifact_and_schema_faults_are_rechecked_after_writes() {
    for which in ["old", "new", "schema", "head", "outbox"] {
        let (f, task, old, new) = pair().await;
        let body = match which {
            "schema" => format!("UPDATE {}.collaboration_artifact_schema SET workspace_id = '{}';", f.artifact_schema, Uuid::new_v4()),
            "head" => format!("UPDATE {}.collaboration_artifacts SET owner_id = '{}';", f.artifact_schema, f.auth.manager.principal.reference.principal_id),
            "outbox" => format!("DELETE FROM {}.collaboration_artifact_outbox;", f.artifact_schema),
            _ => format!("UPDATE {}.collaboration_artifact_revisions SET snapshot = '{{}}' WHERE revision_id = '{}';",
                f.artifact_schema, if which == "old" { old.reference.revision_id } else { new.reference.revision_id }),
        };
        f.fault_trigger(&body).await;
        assert!(
            f.replace(
                &f.auth.learner_token,
                &task,
                manifest("Replacement", vec![new.reference.clone()])
            )
            .await
            .is_err(),
            "{which}"
        );
        assert_eq!(f.fault_calls().await, 1);
        f.unchanged(&task, 1).await;
        for revision in [old, new] {
            assert!(f
                .artifacts
                .read(&revision.reference, revision.owner)
                .await
                .unwrap()
                .is_some());
        }
        assert_eq!(f.files(), 2);
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_consistent_task_tampering_and_session_deletion_cannot_commit() {
    for which in ["task", "session"] {
        let (f, task, _, new) = pair().await;
        let body = if which == "session" {
            format!(
                "DELETE FROM {}.sessions WHERE token = '{}';",
                f.auth.base.schema,
                source_fixture::token_hash(&f.auth.learner_token)
            )
        } else {
            "NEW.snapshot = jsonb_set(NEW.snapshot::jsonb, '{title}', '\"Injected\"')::text;
             NEW.manifest = jsonb_set(NEW.manifest::jsonb, '{title}', '\"Injected\"')::text;
             UPDATE collaboration_tasks SET snapshot = NEW.snapshot, manifest = NEW.manifest WHERE task_id = NEW.task_id;".into()
        };
        f.fault_trigger(&body).await;
        assert_eq!(
            f.replace(
                &f.auth.learner_token,
                &task,
                manifest("Replacement", vec![new.reference])
            )
            .await,
            Err(if which == "session" {
                SessionTaskError::Session(SessionCommandError::InvalidSession)
            } else {
                SessionTaskError::Task(TaskStoreError::InvalidRecord)
            })
        );
        assert_eq!(f.fault_calls().await, 1);
        f.unchanged(&task, 1).await;
        assert!(f
            .auth
            .source
            .validate_session(&f.auth.learner_token)
            .await
            .unwrap()
            .is_some());
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}
