use super::*;
use std::os::unix::fs::PermissionsExt;

async fn empty_tasks(f: &Fixture) {
    assert_eq!(f.count_task("collaboration_tasks").await, 0);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 0);
}

async fn calls(f: &Fixture) -> i64 {
    let row = query("SELECT last_value, is_called FROM input_fault_calls")
        .fetch_one(&f.task_pool)
        .await
        .unwrap();
    assert!(
        row.get::<bool, _>("is_called"),
        "fault trigger was never reached"
    );
    row.get("last_value")
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_artifact_namespace_replacement_preserves_original_pin() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Pinned before task write")
        .await;
    let clone = format!("input_clone_{}", Uuid::new_v4().simple());
    let moved = format!("input_moved_{}", Uuid::new_v4().simple());
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
    // The clone contains identical valid records and uses the same private files. Metadata or
    // content checks alone cannot identify this replacement; the original namespace OID must.
    f.sql_task("CREATE SEQUENCE input_fault_calls").await;
    f.sql_task(&format!(
        "CREATE FUNCTION replace_input_namespace() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.input_fault_calls');
        ALTER SCHEMA {} RENAME TO {moved}; ALTER SCHEMA {clone} RENAME TO {};
        RETURN NEW; END $$",
        f.task_schema, f.artifact_schema, f.artifact_schema
    ))
    .await;
    f.sql_task(
        "CREATE TRIGGER replace_input_namespace BEFORE INSERT ON collaboration_task_outbox
        FOR EACH ROW EXECUTE FUNCTION replace_input_namespace()",
    )
    .await;
    assert_eq!(
        f.auth
            .source
            .create_session_input_task(
                &f.auth.learner_token,
                &f.workspace,
                id(),
                "Namespace substitution",
                vec![artifact.reference.clone()],
            )
            .await,
        Err(SessionTaskError::InvalidNamespace)
    );
    assert_eq!(
        calls(&f).await,
        1,
        "Task write must reach the namespace replacement"
    );
    empty_tasks(&f).await;
    assert_eq!(f.files(), 1);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    let original = f
        .artifacts
        .read(&artifact.reference, artifact.owner)
        .await
        .unwrap()
        .unwrap();
    let copied = clone_store
        .read(&artifact.reference, artifact.owner)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original, copied);
    assert_eq!(original.bytes, b"Pinned before task write");
    let moved_exists: bool =
        query("SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname = $1) AS present")
            .bind(&moved)
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("present");
    assert!(
        !moved_exists,
        "schema rename must roll back with Task and event"
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
async fn postgres_session_task_inputs_valid_task_clone_path_cannot_escape_namespace_checks() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Content is still legitimate")
        .await;
    let clone = format!("input_task_clone_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {clone}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let clone_pool = pool_for(&clone).await;
    TaskStore::new(clone_pool.clone(), scope())
        .install_empty_namespace()
        .await
        .unwrap();
    f.sql_task("CREATE SEQUENCE input_fault_calls").await;
    f.sql_task(&format!("CREATE FUNCTION switch_input_task_path() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.input_fault_calls');
        INSERT INTO {clone}.collaboration_tasks SELECT * FROM {}.collaboration_tasks WHERE task_id = NEW.task_id;
        INSERT INTO {clone}.collaboration_task_outbox (event_id, task_id, revision, actor_id, event_type, snapshot)
            VALUES (NEW.event_id, NEW.task_id, NEW.revision, NEW.actor_id, NEW.event_type, NEW.snapshot);
        PERFORM set_config('search_path', '{clone}', true); RETURN NEW; END $$", f.task_schema, f.task_schema)).await;
    f.sql_task(
        "CREATE TRIGGER switch_input_task_path BEFORE INSERT ON collaboration_task_outbox
        FOR EACH ROW EXECUTE FUNCTION switch_input_task_path()",
    )
    .await;
    assert_eq!(
        f.auth
            .source
            .create_session_input_task(
                &f.auth.learner_token,
                &f.workspace,
                id(),
                "Coherent wrong Task namespace",
                vec![artifact.reference.clone()],
            )
            .await,
        Err(SessionTaskError::InvalidNamespace)
    );
    assert_eq!(calls(&f).await, 1);
    empty_tasks(&f).await;
    for table in ["collaboration_tasks", "collaboration_task_outbox"] {
        let count: i64 = query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&clone_pool)
            .await
            .unwrap()
            .get("n");
        assert_eq!(count, 0);
    }
    assert_eq!(f.files(), 1);
    assert_eq!(
        f.artifacts
            .read(&artifact.reference, artifact.owner)
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"Content is still legitimate"
    );
    f.assert_pool_restored().await;
    clone_pool.close().await;
    query(&format!("DROP SCHEMA {clone} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_post_write_artifact_corruption_cannot_commit() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Checked again after Task mutation")
        .await;
    let schema = &f.artifact_schema;
    f.sql_task("CREATE SEQUENCE input_fault_calls").await;
    for (index, body) in [
        format!(
            "UPDATE {schema}.collaboration_artifact_schema SET workspace_id = '{}'",
            Uuid::new_v4()
        ),
        format!(
            "UPDATE {schema}.collaboration_artifacts SET owner_id = '{}'",
            f.auth.manager.principal.reference.principal_id
        ),
        format!("DELETE FROM {schema}.collaboration_artifact_outbox"),
    ]
    .into_iter()
    .enumerate()
    {
        f.sql_task(&format!("CREATE OR REPLACE FUNCTION corrupt_task_input() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.input_fault_calls'); {body}; RETURN NEW; END $$", f.task_schema)).await;
        f.sql_task(
            "CREATE TRIGGER corrupt_task_input BEFORE INSERT ON collaboration_task_outbox
            FOR EACH ROW EXECUTE FUNCTION corrupt_task_input()",
        )
        .await;
        assert!(matches!(
            f.auth
                .source
                .create_session_input_task(
                    &f.auth.learner_token,
                    &f.workspace,
                    id(),
                    "Do not publish damaged input",
                    vec![artifact.reference.clone()],
                )
                .await,
            Err(SessionTaskError::Artifact(_))
        ));
        assert_eq!(
            calls(&f).await,
            index as i64 + 1,
            "must reach post-validation Task event"
        );
        empty_tasks(&f).await;
        assert_eq!(f.files(), 1);
        assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
        assert_eq!(
            f.artifacts
                .read(&artifact.reference, artifact.owner)
                .await
                .unwrap()
                .unwrap()
                .bytes,
            b"Checked again after Task mutation"
        );
        f.sql_task("DROP TRIGGER corrupt_task_input ON collaboration_task_outbox")
            .await;
    }
    // Exercise the same final-reference guard on a status mutation, not only creation.
    let task = f
        .create(&f.auth.learner_token, vec![artifact.reference.clone()])
        .await;
    f.sql_task(
        "CREATE TRIGGER corrupt_task_input BEFORE INSERT ON collaboration_task_outbox
        FOR EACH ROW EXECUTE FUNCTION corrupt_task_input()",
    )
    .await;
    assert!(matches!(
        f.auth
            .source
            .transition_session_input_task(
                &f.auth.learner_token,
                &f.workspace,
                task.reference.task_id,
                task.revision,
                TaskStatus::Ready,
            )
            .await,
        Err(SessionTaskError::Artifact(_))
    ));
    assert_eq!(calls(&f).await, 4);
    assert_eq!(
        f.tasks.get(task.reference, task.owner).await.unwrap(),
        Some(task)
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    assert_eq!(f.files(), 1);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_task_event_and_commit_faults_leave_files_unchanged() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Existing input is never cleaned up")
        .await;
    f.sql_task("CREATE SEQUENCE input_fault_calls").await;
    f.sql_task(&format!(
        "CREATE FUNCTION suppress_input_task_write() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.input_fault_calls'); RETURN NULL; END $$",
        f.task_schema
    ))
    .await;
    for (index, table) in ["collaboration_tasks", "collaboration_task_outbox"]
        .into_iter()
        .enumerate()
    {
        f.sql_task(&format!(
            "CREATE TRIGGER suppress_input_task_write BEFORE INSERT ON {table}
            FOR EACH ROW EXECUTE FUNCTION suppress_input_task_write()"
        ))
        .await;
        assert!(f
            .auth
            .source
            .create_session_input_task(
                &f.auth.learner_token,
                &f.workspace,
                id(),
                "Suppressed write",
                vec![artifact.reference.clone()],
            )
            .await
            .is_err());
        assert_eq!(calls(&f).await, index as i64 + 1);
        empty_tasks(&f).await;
        assert_eq!(f.files(), 1);
        f.sql_task(&format!(
            "DROP TRIGGER suppress_input_task_write ON {table}"
        ))
        .await;
    }
    let task = f
        .create(&f.auth.learner_token, vec![artifact.reference.clone()])
        .await;
    f.sql_task(&format!("CREATE FUNCTION fail_input_task_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        PERFORM nextval('{}.input_fault_calls'); RAISE EXCEPTION 'synthetic input Task commit failure'; END $$", f.task_schema)).await;
    f.sql_task(
        "CREATE CONSTRAINT TRIGGER fail_input_task_commit AFTER INSERT ON collaboration_task_outbox
        DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_input_task_commit()",
    )
    .await;
    assert_eq!(
        f.auth
            .source
            .transition_session_input_task(
                &f.auth.learner_token,
                &f.workspace,
                task.reference.task_id,
                task.revision,
                TaskStatus::Ready,
            )
            .await,
        Err(SessionTaskError::Session(
            SessionCommandError::Authentication(DurableAuthError::StorageUnavailable)
        ))
    );
    assert_eq!(
        calls(&f).await,
        3,
        "deferred fault must be reached after final admission"
    );
    assert_eq!(
        f.tasks.get(task.reference, task.owner).await.unwrap(),
        Some(task)
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.files(), 1);
    assert_eq!(
        f.artifacts
            .read(&artifact.reference, artifact.owner)
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"Existing input is never cleaned up"
    );
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_corrupt_files_block_read_and_transition_without_repair() {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Original input bytes")
        .await;
    let task = f
        .create(&f.auth.learner_token, vec![artifact.reference.clone()])
        .await;
    let blob = f.blob(&artifact.reference);
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o600)).unwrap();
    let mut changed = b"Original input bytes".to_vec();
    changed[0] ^= 1;
    fs::write(&blob, &changed).unwrap();
    fs::set_permissions(&blob, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(matches!(
        f.auth
            .source
            .get_session_input_task(&f.auth.learner_token, &f.workspace, task.reference.task_id,)
            .await,
        Err(SessionTaskError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    ));
    assert_eq!(
        f.auth
            .source
            .transition_session_input_task(
                &f.auth.learner_token,
                &f.workspace,
                task.reference.task_id,
                task.revision,
                TaskStatus::Ready,
            )
            .await,
        Err(SessionTaskError::Artifact(
            ArtifactStoreError::BlobUnavailable
        ))
    );
    assert_eq!(
        f.tasks.get(task.reference, task.owner).await.unwrap(),
        Some(task)
    );
    assert_eq!(f.count_task("collaboration_task_outbox").await, 1);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    assert_eq!(f.files(), 1);
    assert_eq!(fs::read(blob).unwrap(), changed);
    f.assert_pool_restored().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_task_inputs_post_write_session_deletion_rolls_back_each_mutation() {
    for transition in [false, true] {
        let f = Fixture::ready().await;
        let artifact = f
            .create_artifact(
                &f.auth.learner_token,
                b"Input survives final Session refusal",
            )
            .await;
        let original = if transition {
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
        f.sql_task("CREATE SEQUENCE input_fault_calls").await;
        f.sql_task(&format!(
            "CREATE FUNCTION delete_input_task_session() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
            PERFORM nextval('{}.input_fault_calls');
            DELETE FROM {}.sessions WHERE token = '{}'; RETURN NEW; END $$",
            f.task_schema,
            f.auth.base.schema,
            source_fixture::token_hash(&f.auth.learner_token)
        ))
        .await;
        f.sql_task(
            "CREATE TRIGGER delete_input_task_session BEFORE INSERT ON collaboration_task_outbox
            FOR EACH ROW EXECUTE FUNCTION delete_input_task_session()",
        )
        .await;
        let result = match &original {
            Some(task) => {
                f.auth
                    .source
                    .transition_session_input_task(
                        &f.auth.learner_token,
                        &f.workspace,
                        task.reference.task_id,
                        task.revision,
                        TaskStatus::Ready,
                    )
                    .await
            }
            None => {
                f.auth
                    .source
                    .create_session_input_task(
                        &f.auth.learner_token,
                        &f.workspace,
                        id(),
                        "Refused after Task and input checks",
                        vec![artifact.reference.clone()],
                    )
                    .await
            }
        };
        assert_eq!(
            result,
            Err(SessionTaskError::Session(
                SessionCommandError::InvalidSession
            ))
        );
        assert_eq!(
            calls(&f).await,
            1,
            "must reach the Task event after initial admission"
        );
        assert_eq!(
            f.auth
                .source
                .validate_session(&f.auth.learner_token)
                .await
                .unwrap(),
            Some(session),
            "same-transaction Session deletion must roll back too"
        );
        assert_eq!(
            f.count_task("collaboration_tasks").await,
            i64::from(transition)
        );
        assert_eq!(
            f.count_task("collaboration_task_outbox").await,
            i64::from(transition)
        );
        if let Some(task) = original {
            assert_eq!(
                f.tasks.get(task.reference, task.owner).await.unwrap(),
                Some(task)
            );
        }
        assert_eq!(f.count_artifact("collaboration_artifacts").await, 1);
        assert_eq!(
            f.count_artifact("collaboration_artifact_revisions").await,
            1
        );
        assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
        assert_eq!(f.files(), 1);
        let content = f
            .artifacts
            .read(&artifact.reference, artifact.owner)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(content.revision, artifact);
        assert_eq!(content.bytes, b"Input survives final Session refusal");
        f.assert_pool_restored().await;
        f.cleanup().await;
    }
}
