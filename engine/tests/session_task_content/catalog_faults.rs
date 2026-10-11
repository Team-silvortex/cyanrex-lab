use super::catalog as c;
use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_catalog_post_write_definition_artifact_event_and_session_faults_roll_back(
) {
    for which in ["definition", "old", "new", "event", "deferred", "session"] {
        let (f, workspace, task, old, new) = c::pair().await;
        // Both pins belong to the original Task, so a transition also verifies the "new" pin.
        let task = c::replace(
            &f,
            &workspace,
            &task,
            manifest(
                "Two saved pins",
                vec![old.reference.clone(), new.reference.clone()],
            ),
        )
        .await
        .unwrap();
        let before = c::business_rows(&f).await;
        if which == "deferred" {
            f.sql_task("CREATE SEQUENCE content_fault_calls").await;
            f.sql_task(&format!("CREATE FUNCTION content_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                PERFORM nextval('{}.content_fault_calls'); RAISE EXCEPTION 'synthetic catalog commit fault'; END $$", f.task_schema)).await;
            f.sql_task(
                "CREATE CONSTRAINT TRIGGER content_fault AFTER INSERT ON collaboration_task_outbox
                DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION content_fault()",
            )
            .await;
        } else {
            let body = match which {
                "definition" => "NEW.snapshot = jsonb_set(NEW.snapshot::jsonb, '{definition,summary}', '\"Injected summary\"')::text;
                    UPDATE collaboration_tasks SET snapshot = NEW.snapshot WHERE task_id = NEW.task_id;".to_string(),
                "old" | "new" => format!("UPDATE {}.collaboration_artifact_revisions SET snapshot = '{{}}' WHERE revision_id = '{}';",
                    f.artifact_schema, if which == "old" { old.reference.revision_id } else { new.reference.revision_id }),
                "event" => "RETURN NULL;".into(),
                "session" => format!("DELETE FROM {}.sessions WHERE token = '{}';", f.auth.base.schema, source_fixture::token_hash(&f.auth.learner_token)),
                _ => unreachable!(),
            };
            f.fault_trigger(&body).await;
        }
        assert!(
            c::replace(
                &f,
                &workspace,
                &task,
                manifest("Catalog replacement", vec![new.reference.clone()])
            )
            .await
            .is_err(),
            "{which}"
        );
        assert!(
            c::transition(&f, &workspace, &task, TaskStatus::Ready)
                .await
                .is_err(),
            "{which}"
        );
        assert!(
            f.auth
                .source
                .create_session_catalog_content_task(
                    &f.auth.learner_token,
                    &workspace,
                    id(),
                    &c::definition(1).reference,
                    manifest(
                        "Catalog creation",
                        vec![old.reference.clone(), new.reference.clone()]
                    ),
                )
                .await
                .is_err(),
            "{which}"
        );
        assert_eq!(f.fault_calls().await, 3, "{which}");
        assert_eq!(c::business_rows(&f).await, before, "{which}");
        assert_eq!(
            c::get(&f, &workspace, &task)
                .await
                .unwrap()
                .unwrap()
                .snapshot,
            task
        );
        assert!(f
            .auth
            .source
            .validate_session(&f.auth.learner_token)
            .await
            .unwrap()
            .is_some());
        assert_eq!(f.files(), 2);
        assert_eq!(fs::read(f.blob(&old.reference)).unwrap(), b"Original input");
        assert_eq!(
            fs::read(f.blob(&new.reference)).unwrap(),
            b"Replacement input"
        );
        f.assert_pool_restored().await;
        drop(workspace);
        f.cleanup().await;
    }
}
