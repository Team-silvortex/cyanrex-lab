use super::*;

async fn closed_pool_reads_keep_the_selected_backend(schema_ready: bool) {
    let fixture = Fixture::new();
    let seeded = fixture.store(&[attempt("alice", 0)]).await;
    let original = std::fs::read(&seeded.data_path).unwrap();
    // No connection is made: the lazy pool is closed before any query or store use.
    let pool = PgPoolOptions::new()
        .min_connections(0)
        .connect_lazy("postgres://synthetic:synthetic@127.0.0.1:1/unused")
        .unwrap();
    pool.close().await;
    for warm_local_snapshot in [false, true] {
        let mut store = LearningStore::with_local_data_path(seeded.data_path.clone());
        if warm_local_snapshot {
            store.load_memory().await.unwrap();
        }
        store.db_pool = Some(pool.clone());
        if schema_ready {
            store.schema_ready.set(()).unwrap();
        }
        assert!(
            store.active_pool().is_some(),
            "test requires DB fallback enabled"
        );
        let before = store.in_memory.read().await.clone();
        for _ in 0..2 {
            assert!(store.attempts_for_user("alice").await.is_err());
            assert!(store.recent_attempts_for_user("alice", 20).await.is_err());
            assert!(store.progress_for_user("alice").await.is_err());
            assert!(store.teacher_overview().await.is_err());
            assert!(!store.db_disabled.load(Ordering::Relaxed));
            assert_eq!(store.memory_loaded.initialized(), warm_local_snapshot);
            assert_eq!(store.schema_ready.initialized(), schema_ready);
            assert!(Arc::ptr_eq(&before, &*store.in_memory.read().await));
            assert_eq!(std::fs::read(&store.data_path).unwrap(), original);
        }
    }
}

#[tokio::test]
async fn schema_read_failure_is_not_empty_or_stale_local_success() {
    closed_pool_reads_keep_the_selected_backend(false).await;
}

#[tokio::test]
async fn query_read_failure_is_not_empty_or_stale_local_success() {
    closed_pool_reads_keep_the_selected_backend(true).await;
}

#[tokio::test]
async fn a_new_local_classroom_still_has_legitimate_empty_reads_without_writing() {
    let fixture = Fixture::new();
    let store = LearningStore::with_local_data_path(fixture.0.join("not-created.json"));
    assert!(store.attempts_for_user("alice").await.unwrap().is_empty());
    assert!(store
        .recent_attempts_for_user("alice", 20)
        .await
        .unwrap()
        .is_empty());
    let labs = store.progress_for_user("alice").await.unwrap();
    assert!(!labs.is_empty());
    assert!(labs.iter().all(|lab| lab.attempts == 0));
    let overview = store.teacher_overview().await.unwrap();
    assert_eq!(overview.active_students, 0);
    assert!(overview.students.is_empty());
    assert!(!store.data_path.exists());
}
