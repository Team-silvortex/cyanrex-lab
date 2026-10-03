use super::*;

async fn calls(f: &Fixture) -> i64 {
    let row = query("SELECT last_value, is_called FROM catalog_fault_calls")
        .fetch_one(&f.task_pool)
        .await
        .unwrap();
    assert!(
        row.get::<bool, _>("is_called"),
        "fault trigger was never reached"
    );
    row.get("last_value")
}

async fn unchanged_task(f: &Fixture, before: Option<&TaskSnapshot>) {
    for table in ["collaboration_tasks", "collaboration_task_outbox"] {
        assert_eq!(f.count_task(table).await, i64::from(before.is_some()));
    }
    if let Some(before) = before {
        assert_eq!(
            f.tasks.get(before.reference, before.owner).await.unwrap(),
            Some(before.clone())
        );
    }
}

async fn unchanged_artifact(f: &Fixture, artifact: &ArtifactRevision, bytes: &[u8]) {
    let content = f
        .artifacts
        .read(&artifact.reference, artifact.owner)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&content.revision, artifact);
    assert_eq!(content.bytes, bytes);
    for table in [
        "collaboration_artifacts",
        "collaboration_artifact_revisions",
        "collaboration_artifact_outbox",
    ] {
        assert_eq!(f.count_artifact(table).await, 1);
    }
    assert_eq!(f.files(), 1);
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_post_write_definition_drift_cannot_publish() {
    for transition in [false, true] {
        let f = Fixture::ready().await;
        let artifact = f
            .create_artifact(&f.auth.learner_token, b"Definition-pinned input")
            .await;
        let before = if transition {
            Some(
                f.create(&f.auth.learner_token, vec![artifact.reference.clone()])
                    .await,
            )
        } else {
            None
        };
        f.sql_task("CREATE SEQUENCE catalog_fault_calls").await;
        f.sql_task(&format!("CREATE FUNCTION drift_catalog_snapshot() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.catalog_fault_calls');
            NEW.snapshot = jsonb_set(NEW.snapshot::jsonb, '{{definition,summary}}', to_jsonb('Different definition under the same pin'::text))::text;
            UPDATE {}.collaboration_tasks SET snapshot = NEW.snapshot WHERE task_id = NEW.task_id;
            RETURN NEW; END $$", f.task_schema, f.task_schema)).await;
        f.sql_task("CREATE TRIGGER drift_catalog_snapshot BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION drift_catalog_snapshot()").await;
        let result = match &before {
            Some(task) => {
                f.transition(&f.auth.learner_token, task, TaskStatus::Ready)
                    .await
            }
            None => {
                f.request(
                    &f.auth.learner_token,
                    id(),
                    vec![artifact.reference.clone()],
                )
                .await
            }
        };
        // The stored head/event agree and remain structurally valid, but differ from the pending
        // snapshot. TaskStore's own readback rejects first; do not claim this reaches the outer
        // configured-definition mismatch check merely because the command failed.
        assert_eq!(
            result,
            Err(SessionTaskError::Task(TaskStoreError::InvalidRecord))
        );
        assert_eq!(calls(&f).await, 1);
        unchanged_task(&f, before.as_ref()).await;
        unchanged_artifact(&f, &artifact, b"Definition-pinned input").await;
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_post_write_inputs_and_session_faults_roll_back() {
    for transition in [false, true] {
        let f = Fixture::ready().await;
        let artifact = f
            .create_artifact(&f.auth.learner_token, b"Check after catalogue Task writes")
            .await;
        let before = if transition {
            Some(
                f.create(&f.auth.learner_token, vec![artifact.reference.clone()])
                    .await,
            )
        } else {
            None
        };
        let session = f
            .auth
            .source
            .validate_session(&f.auth.learner_token)
            .await
            .unwrap()
            .unwrap();
        f.sql_task("CREATE SEQUENCE catalog_fault_calls").await;
        for (index, (session_fault, body)) in [
            (
                false,
                format!(
                    "UPDATE {}.collaboration_artifact_schema SET workspace_id = '{}'",
                    f.artifact_schema,
                    Uuid::new_v4()
                ),
            ),
            (
                false,
                format!(
                    "UPDATE {}.collaboration_artifacts SET owner_id = '{}'",
                    f.artifact_schema, f.auth.manager.principal.reference.principal_id
                ),
            ),
            (
                false,
                format!(
                    "DELETE FROM {}.collaboration_artifact_outbox",
                    f.artifact_schema
                ),
            ),
            (
                true,
                format!(
                    "DELETE FROM {}.sessions WHERE token = '{}'",
                    f.auth.base.schema,
                    source_fixture::token_hash(&f.auth.learner_token)
                ),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            f.sql_task(&format!("CREATE OR REPLACE FUNCTION corrupt_catalog_context() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
                PERFORM nextval('{}.catalog_fault_calls'); {body}; RETURN NEW; END $$", f.task_schema)).await;
            f.sql_task("CREATE TRIGGER corrupt_catalog_context BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION corrupt_catalog_context()").await;
            let result = match &before {
                Some(task) => {
                    f.transition(&f.auth.learner_token, task, TaskStatus::Ready)
                        .await
                }
                None => {
                    f.request(
                        &f.auth.learner_token,
                        id(),
                        vec![artifact.reference.clone()],
                    )
                    .await
                }
            };
            if session_fault {
                assert_eq!(
                    result,
                    Err(SessionTaskError::Session(
                        SessionCommandError::InvalidSession
                    ))
                );
            } else {
                assert!(
                    matches!(result, Err(SessionTaskError::Artifact(_))),
                    "{result:?}"
                );
            }
            assert_eq!(
                calls(&f).await,
                index as i64 + 1,
                "must reach Task event after original definition/content admission"
            );
            unchanged_task(&f, before.as_ref()).await;
            assert_eq!(
                f.auth
                    .source
                    .validate_session(&f.auth.learner_token)
                    .await
                    .unwrap(),
                Some(session.clone())
            );
            unchanged_artifact(&f, &artifact, b"Check after catalogue Task writes").await;
            f.sql_task("DROP TRIGGER corrupt_catalog_context ON collaboration_task_outbox")
                .await;
        }
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_artifact_replacement_preserves_original_namespace_pin() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Pinned catalogue input")
        .await;
    let clone = format!("catalog_artifact_clone_{}", Uuid::new_v4().simple());
    let moved = format!("catalog_artifact_moved_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {clone}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let clone_pool = pool_for(&clone).await;
    let clone_store = ArtifactStore::open(clone_pool.clone(), scope(), &f.directory).unwrap();
    clone_store.install_empty_namespace().await.unwrap();
    for table in [
        "collaboration_artifacts",
        "collaboration_artifact_revisions",
        "collaboration_artifact_outbox",
    ] {
        query(&format!(
            "INSERT INTO {clone}.{table} SELECT * FROM {}.{table}",
            f.artifact_schema
        ))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    }
    // The substitute has valid identical records and the same private files. Rechecking only
    // metadata and bytes would pass: the originally observed namespace OID must be retained.
    f.sql_task("CREATE SEQUENCE catalog_fault_calls").await;
    f.sql_task(&format!(
        "CREATE FUNCTION replace_catalog_artifacts() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.catalog_fault_calls'); ALTER SCHEMA {} RENAME TO {moved};
        ALTER SCHEMA {clone} RENAME TO {}; RETURN NEW; END $$",
        f.task_schema, f.artifact_schema, f.artifact_schema
    ))
    .await;
    f.sql_task("CREATE TRIGGER replace_catalog_artifacts BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION replace_catalog_artifacts()").await;
    assert_eq!(
        f.request(
            &f.auth.learner_token,
            id(),
            vec![artifact.reference.clone()]
        )
        .await,
        Err(SessionTaskError::InvalidNamespace)
    );
    assert_eq!(calls(&f).await, 1);
    unchanged_task(&f, None).await;
    unchanged_artifact(&f, &artifact, b"Pinned catalogue input").await;
    assert_eq!(
        clone_store
            .read(&artifact.reference, artifact.owner)
            .await
            .unwrap()
            .unwrap()
            .revision,
        artifact
    );
    let moved_exists: bool =
        query("SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname = $1) AS present")
            .bind(&moved)
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("present");
    assert!(
        !moved_exists,
        "same-transaction namespace replacement must roll back"
    );
    f.assert_pool_restored().await;
    clone_pool.close().await;
    drop(clone_store);
    query(&format!("DROP SCHEMA {clone} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_catalog_tasks_zero_input_post_write_scope_changes_roll_back() {
    for transition in [false, true] {
        let f = Fixture::ready().await;
        let before = if transition {
            Some(f.create(&f.auth.learner_token, vec![]).await)
        } else {
            None
        };
        f.sql_task("CREATE SEQUENCE catalog_fault_calls").await;
        f.sql_task(&format!("CREATE FUNCTION corrupt_empty_catalog_scope() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.catalog_fault_calls'); UPDATE {}.collaboration_artifact_schema SET workspace_id = '{}';
            RETURN NEW; END $$", f.task_schema, f.artifact_schema, Uuid::new_v4())).await;
        f.sql_task("CREATE TRIGGER corrupt_empty_catalog_scope BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION corrupt_empty_catalog_scope()").await;
        let result = match &before {
            Some(task) => {
                f.transition(&f.auth.learner_token, task, TaskStatus::Ready)
                    .await
            }
            None => f.request(&f.auth.learner_token, id(), vec![]).await,
        };
        assert_eq!(
            result,
            Err(SessionTaskError::Artifact(
                ArtifactStoreError::ScopeMismatch
            ))
        );
        assert_eq!(
            calls(&f).await,
            1,
            "initial metadata admission must succeed before the fault"
        );
        unchanged_task(&f, before.as_ref()).await;
        let actual: Uuid =
            query("SELECT workspace_id FROM collaboration_artifact_schema WHERE singleton")
                .fetch_one(&f.artifact_pool)
                .await
                .unwrap()
                .get("workspace_id");
        assert_eq!(actual, scope().workspace_id.as_uuid());
        for table in [
            "collaboration_artifacts",
            "collaboration_artifact_revisions",
            "collaboration_artifact_outbox",
        ] {
            assert_eq!(f.count_artifact(table).await, 0);
        }
        assert_eq!(f.files(), 0);
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}
