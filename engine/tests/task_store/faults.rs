use super::*;

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_temporary_shadow_tables_are_not_authoritative() {
    let f = Fixture::ready().await;
    f.store
        .create(reference().task_id, owner(), manual())
        .await
        .unwrap();
    let schema = f.schema.clone();
    let url = std::env::var("CYANREX_TEST_DATABASE_URL").unwrap();
    let pool = PgPoolOptions::new().max_connections(1).after_connect(move |connection, _| {
        let schema = schema.clone();
        Box::pin(async move {
            query("SELECT set_config('search_path', $1, false)").bind(schema).execute(&mut *connection).await?;
            query("CREATE TEMP TABLE collaboration_tasks (LIKE collaboration_tasks INCLUDING ALL)").execute(connection).await?;
            Ok(())
        })
    }).connect(&url).await.unwrap();
    let store = TaskStore::new(pool.clone(), scope());
    assert_eq!(
        store.get(reference(), owner()).await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    assert_eq!(
        store.create(id(), owner(), manual()).await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    pool.close().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_post_write_scope_changes_roll_back_before_success() {
    let f = Fixture::ready().await;
    f.sql("CREATE FUNCTION change_scope() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN
        UPDATE collaboration_task_schema SET workspace_id = '267c6b0f-182c-4980-a925-cdc967327c57'; RETURN NEW; END $$").await;
    f.sql("CREATE TRIGGER change_scope BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION change_scope()").await;
    assert!(f
        .store
        .create(reference().task_id, owner(), manual())
        .await
        .is_err());
    assert_eq!(f.count("collaboration_tasks").await, 0);
    assert_eq!(f.count("collaboration_task_outbox").await, 0);
    assert_eq!(f.store.get(reference(), owner()).await.unwrap(), None);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_install_only_explicit_empty_namespaces() {
    let f = Fixture::new().await;
    assert!(f.store.get(reference(), owner()).await.is_err());
    let objects: i64 = query(
        "SELECT count(*) AS n FROM pg_class WHERE relnamespace = current_schema()::regnamespace",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap()
    .get("n");
    assert_eq!(objects, 0);
    f.sql("CREATE TABLE unrelated (value TEXT)").await;
    assert_eq!(
        f.store.install_empty_namespace().await,
        Err(TaskStoreError::NamespaceNotEmpty)
    );
    f.sql("DROP TABLE unrelated").await;
    f.store.install_empty_namespace().await.unwrap();
    assert_eq!(
        f.store.install_empty_namespace().await,
        Err(TaskStoreError::NamespaceNotEmpty)
    );
    f.sql("UPDATE collaboration_task_schema SET version = 99")
        .await;
    assert_eq!(
        f.store.get(reference(), owner()).await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_failed_or_suppressed_events_roll_back_the_task() {
    let f = Fixture::ready().await;
    f.sql("CREATE FUNCTION suppress_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END $$").await;
    f.sql("CREATE TRIGGER suppress_event BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION suppress_event()").await;
    assert!(f
        .store
        .create(reference().task_id, owner(), manual())
        .await
        .is_err());
    assert_eq!(f.count("collaboration_tasks").await, 0);
    f.sql("DROP TRIGGER suppress_event ON collaboration_task_outbox")
        .await;
    let original = f
        .store
        .create(reference().task_id, owner(), manual())
        .await
        .unwrap();
    f.sql("CREATE TRIGGER suppress_event BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION suppress_event()").await;
    assert!(f
        .store
        .transition(reference(), owner(), rev(1), TaskStatus::Ready)
        .await
        .is_err());
    assert_eq!(
        f.store.get(reference(), owner()).await.unwrap(),
        Some(original)
    );
    assert_eq!(f.count("collaboration_task_outbox").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_state_or_event_tampering_is_rejected_before_publication() {
    let f = Fixture::ready().await;
    f.sql("CREATE FUNCTION alter_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.snapshot = '{}'; RETURN NEW; END $$").await;
    f.sql("CREATE TRIGGER alter_event BEFORE INSERT ON collaboration_task_outbox FOR EACH ROW EXECUTE FUNCTION alter_event()").await;
    assert!(f
        .store
        .create(reference().task_id, owner(), manual())
        .await
        .is_err());
    assert_eq!(f.count("collaboration_tasks").await, 0);
    f.sql("DROP TRIGGER alter_event ON collaboration_task_outbox")
        .await;
    f.store
        .create(reference().task_id, owner(), manual())
        .await
        .unwrap();
    f.sql("UPDATE collaboration_tasks SET snapshot = jsonb_set(snapshot::jsonb, '{title}', '\"tampered\"')::text").await;
    assert_eq!(
        f.store.get(reference(), owner()).await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::InvalidRecord)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_deferred_commit_failure_returns_no_success_or_partial_event() {
    let f = Fixture::ready().await;
    f.sql("CREATE FUNCTION fail_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic commit failure'; END $$").await;
    f.sql("CREATE CONSTRAINT TRIGGER fail_commit AFTER INSERT ON collaboration_task_outbox DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_commit()").await;
    assert_eq!(
        f.store.create(reference().task_id, owner(), manual()).await,
        Err(TaskStoreError::StorageUnavailable)
    );
    assert_eq!(f.count("collaboration_tasks").await, 0);
    assert_eq!(f.count("collaboration_task_outbox").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_cancelled_write_before_event_commit_is_rolled_back() {
    let f = Fixture::ready().await;
    let blocker = f.pause_events().await;
    let store = f.store.clone();
    let pending =
        tokio::spawn(async move { store.create(reference().task_id, owner(), manual()).await });
    f.wait_blocked().await;
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    blocker.rollback().await.unwrap();
    // A retry with the same stable task ID waits for rollback if the connection is still cleaning up.
    f.store
        .create(reference().task_id, owner(), manual())
        .await
        .unwrap();
    assert_eq!(f.count("collaboration_tasks").await, 1);
    assert_eq!(f.count("collaboration_task_outbox").await, 1);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_missing_event_and_rls_tables_fail_closed() {
    let f = Fixture::ready().await;
    f.store
        .create(reference().task_id, owner(), manual())
        .await
        .unwrap();
    f.sql("ALTER TABLE collaboration_tasks ENABLE ROW LEVEL SECURITY")
        .await;
    assert_eq!(
        f.store.get(reference(), owner()).await,
        Err(TaskStoreError::UnsupportedSchema)
    );
    f.sql("ALTER TABLE collaboration_tasks DISABLE ROW LEVEL SECURITY")
        .await;
    f.sql("DELETE FROM collaboration_task_outbox").await;
    assert_eq!(
        f.store.get(reference(), owner()).await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(
        f.store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await,
        Err(TaskStoreError::InvalidRecord)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_readers_observe_one_committed_snapshot_during_a_write() {
    let f = Fixture::ready().await;
    let before = f
        .store
        .create(reference().task_id, owner(), manual())
        .await
        .unwrap();
    let blocker = f.pause_events().await;
    let store = f.store.clone();
    let pending = tokio::spawn(async move {
        store
            .transition(reference(), owner(), rev(1), TaskStatus::Ready)
            .await
    });
    f.wait_blocked().await;
    let observed = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        f.store.get(reference(), owner()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(observed, Some(before));
    blocker.rollback().await.unwrap();
    let after = pending.await.unwrap().unwrap();
    assert_eq!(after.revision, rev(2));
    assert_eq!(
        f.store.get(reference(), owner()).await.unwrap(),
        Some(after)
    );
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_writers_wait_for_metadata_before_admission() {
    let f = Fixture::ready().await;
    let mut blocker = f.pool.begin().await.unwrap();
    query("UPDATE collaboration_task_schema SET version = 99")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let store = f.store.clone();
    let pending =
        tokio::spawn(async move { store.create(reference().task_id, owner(), manual()).await });
    f.wait_blocked().await;
    blocker.commit().await.unwrap();
    assert_eq!(
        pending.await.unwrap(),
        Err(TaskStoreError::UnsupportedSchema)
    );
    assert_eq!(f.count("collaboration_tasks").await, 0);
    assert_eq!(f.count("collaboration_task_outbox").await, 0);
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requires a disposable CYANREX_TEST_DATABASE_URL"]
async fn postgres_tasks_revision_overflow_does_not_wrap_or_append_an_event() {
    let f = Fixture::ready().await;
    f.store
        .create(reference().task_id, owner(), manual())
        .await
        .unwrap();
    f.sql("UPDATE collaboration_tasks SET revision = 9007199254740991,
        snapshot = jsonb_set(jsonb_set(snapshot::jsonb, '{revision}', '9007199254740991'), '{status}', '\"ready\"')::text").await;
    f.sql("UPDATE collaboration_task_outbox SET revision = 9007199254740991,
        snapshot = (SELECT snapshot FROM collaboration_tasks), event_type = 'cyanrex.task.status_changed'").await;
    assert_eq!(
        f.store
            .transition(
                reference(),
                owner(),
                rev(9_007_199_254_740_991),
                TaskStatus::InProgress
            )
            .await,
        Err(TaskStoreError::InvalidRecord)
    );
    assert_eq!(f.count("collaboration_task_outbox").await, 1);
    f.cleanup().await;
}
