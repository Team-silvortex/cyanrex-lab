use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_catalog_http_post_write_definition_bytes_session_and_commit_faults_stay_unconfirmed(
) {
    for mode in [
        "definition",
        "old",
        "new",
        "event",
        "deferred",
        "session",
        "membership",
    ] {
        let (f, workspace, route, task, old, new) = pair().await;
        if mode == "deferred" {
            f.sql_task("CREATE SEQUENCE content_fault_calls").await;
            f.sql_task(&format!("CREATE FUNCTION content_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM nextval('{}.content_fault_calls'); RAISE EXCEPTION 'private synthetic commit fault'; END $$",f.task_schema)).await;
            f.sql_task("CREATE CONSTRAINT TRIGGER content_fault AFTER INSERT ON collaboration_task_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION content_fault()").await;
        } else {
            let body = match mode {
                "definition"=> "NEW.snapshot = jsonb_set(NEW.snapshot::jsonb, '{definition,summary}', '\"Injected summary\"')::text; UPDATE collaboration_tasks SET snapshot = NEW.snapshot WHERE task_id = NEW.task_id;".to_string(),
                "old"|"new"=>format!("UPDATE {}.collaboration_artifact_revisions SET snapshot = '{{}}' WHERE revision_id = '{}';",f.artifact_schema,if mode=="old" {old.reference.revision_id}else{new.reference.revision_id}),
                "event"=>"RETURN NULL;".into(),
                "session"=>format!("DELETE FROM {}.sessions WHERE token = '{}';",f.auth.base.schema,source_fixture::token_hash(&f.auth.learner_token)),
                "membership"=>format!("UPDATE {}.collaboration_memberships SET status = 'suspended' WHERE principal_id = '{}';",f.auth.base.schema,f.auth.learner.binding.principal_id),
                _=>unreachable!(),
            };
            f.fault_trigger(&body).await;
        }
        let path = task_path(task.task.reference.task_id);
        let mut requests = vec![
            request(
                "POST",
                COLLECTION,
                &f.auth.learner_token,
                create_body(
                    id(),
                    &values()[0].reference,
                    &manifest(
                        "Failed creation",
                        vec![old.reference.clone(), new.reference.clone()],
                    ),
                ),
            ),
            request(
                "PUT",
                &format!("{path}/content"),
                &f.auth.learner_token,
                replace_body(
                    &task,
                    &manifest("Remove old and add only new", vec![new.reference.clone()]),
                ),
            ),
        ];
        if mode != "new" {
            requests.push(request(
                "POST",
                &format!("{path}/status"),
                &f.auth.learner_token,
                status_body(&task, "ready"),
            ));
        }
        for req in requests {
            let (status, code) = if mode == "session" {
                (StatusCode::UNAUTHORIZED, "invalid_session")
            } else {
                (StatusCode::SERVICE_UNAVAILABLE, "service_unavailable")
            };
            denied(&f, &route, req, status, code).await;
        }
        assert_eq!(
            f.fault_calls().await,
            if mode == "new" { 2 } else { 3 },
            "all mutations reached the injected {mode} fault"
        );
        assert!(f
            .auth
            .source
            .validate_session(&f.auth.learner_token)
            .await
            .unwrap()
            .is_some());
        read(&f, &route, &f.auth.learner_token, &task).await;
        f.assert_pool_restored().await;
        drop((route, workspace));
        f.cleanup().await;
    }
}
