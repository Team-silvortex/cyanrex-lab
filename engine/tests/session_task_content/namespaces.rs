use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_artifact_namespace_clone_cannot_replace_the_original_pin() {
    let (f, task, old, new) = pair().await;
    let clone = format!("content_artifact_clone_{}", Uuid::new_v4().simple());
    let moved = format!("content_artifact_moved_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {clone}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let pool = input_fixture::pool_for(&clone).await;
    let store = ArtifactStore::open(pool.clone(), scope(), &f.directory).unwrap();
    store.install_empty_namespace().await.unwrap();
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
    f.fault_trigger(&format!(
        "ALTER SCHEMA {} RENAME TO {moved}; ALTER SCHEMA {clone} RENAME TO {};",
        f.artifact_schema, f.artifact_schema
    ))
    .await;
    assert_eq!(
        f.replace(
            &f.auth.learner_token,
            &task,
            manifest("Replacement", vec![new.reference.clone()])
        )
        .await,
        Err(SessionTaskError::InvalidNamespace)
    );
    assert_eq!(f.fault_calls().await, 1);
    f.unchanged(&task, 1).await;
    let present: bool =
        query("SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname = $1) AS present")
            .bind(&moved)
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("present");
    assert!(!present);
    assert_eq!(f.files(), 2);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 2);
    for revision in [old, new] {
        let original = f
            .artifacts
            .read(&revision.reference, revision.owner)
            .await
            .unwrap()
            .unwrap();
        let copied = store
            .read(&revision.reference, revision.owner)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            original, copied,
            "both valid namespaces must survive the rejected substitution"
        );
    }
    f.assert_pool_restored().await;
    pool.close().await;
    drop(store);
    query(&format!("DROP SCHEMA {clone} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_valid_task_clone_search_path_cannot_publish_a_different_namespace(
) {
    let f = Fixture::ready().await;
    let artifact = f
        .create_artifact(&f.auth.learner_token, b"Still legitimate text")
        .await;
    let clone = format!("content_task_clone_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {clone}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let pool = input_fixture::pool_for(&clone).await;
    TaskContentStore::new(pool.clone(), scope())
        .install_empty_namespace()
        .await
        .unwrap();
    f.fault_trigger(&format!("INSERT INTO {clone}.collaboration_tasks SELECT * FROM {}.collaboration_tasks WHERE task_id = NEW.task_id;
        INSERT INTO {clone}.collaboration_task_outbox (event_id, task_id, revision, actor_id, event_type, snapshot, manifest)
            VALUES (NEW.event_id, NEW.task_id, NEW.revision, NEW.actor_id, NEW.event_type, NEW.snapshot, NEW.manifest);
        PERFORM set_config('search_path', '{clone}', true);", f.task_schema)).await;
    assert_eq!(
        f.request(
            &f.auth.learner_token,
            id(),
            manifest("Wrong namespace", vec![artifact.reference.clone()])
        )
        .await,
        Err(SessionTaskError::InvalidNamespace)
    );
    assert_eq!(f.fault_calls().await, 1);
    assert_eq!(f.count_task("collaboration_tasks").await, 0);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 0);
    for table in ["collaboration_tasks", "collaboration_task_outbox"] {
        let count: i64 = query(&format!("SELECT count(*) AS n FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("n");
        assert_eq!(count, 0);
    }
    assert_eq!(f.files(), 1);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 1);
    assert_eq!(
        f.artifacts
            .read(&artifact.reference, artifact.owner)
            .await
            .unwrap()
            .unwrap()
            .bytes,
        b"Still legitimate text"
    );
    f.assert_pool_restored().await;
    pool.close().await;
    query(&format!("DROP SCHEMA {clone} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_session_content_empty_task_pins_artifact_namespace_before_post_write_revisit() {
    let f = Fixture::ready().await;
    let clone = format!("content_empty_clone_{}", Uuid::new_v4().simple());
    let moved = format!("content_empty_moved_{}", Uuid::new_v4().simple());
    query(&format!("CREATE SCHEMA {clone}"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    let pool = input_fixture::pool_for(&clone).await;
    let store = ArtifactStore::open(pool.clone(), scope(), &f.directory).unwrap();
    store.install_empty_namespace().await.unwrap();
    // There are no references to force an Artifact read. The first namespace visit still has
    // to establish an OID pin, or the post-write visit could adopt this valid empty substitute.
    f.fault_trigger(&format!(
        "ALTER SCHEMA {} RENAME TO {moved}; ALTER SCHEMA {clone} RENAME TO {};",
        f.artifact_schema, f.artifact_schema
    ))
    .await;
    assert_eq!(
        f.request(&f.auth.learner_token, id(), manifest("No payload", vec![]))
            .await,
        Err(SessionTaskError::InvalidNamespace)
    );
    assert_eq!(f.fault_calls().await, 1);
    assert_eq!(f.count_task("collaboration_tasks").await, 0);
    assert_eq!(f.count_task("collaboration_task_outbox").await, 0);
    assert_eq!(f.count_artifact("collaboration_artifact_outbox").await, 0);
    assert_eq!(f.files(), 0);
    let moved_exists: bool =
        query("SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname = $1) AS present")
            .bind(&moved)
            .fetch_one(&f.auth.base.admin)
            .await
            .unwrap()
            .get("present");
    assert!(
        !moved_exists,
        "namespace rename must roll back with the rejected Task creation"
    );
    f.assert_pool_restored().await;
    pool.close().await;
    drop(store);
    query(&format!("DROP SCHEMA {clone} CASCADE"))
        .execute(&f.auth.base.admin)
        .await
        .unwrap();
    f.cleanup().await;
}
