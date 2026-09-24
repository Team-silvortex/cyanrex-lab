use super::*;

#[tokio::test]
async fn cancelled_retention_update_never_splits_settings_history_and_unread() {
    let bus = memory_bus();
    bus.update_settings_for_user("alice", 100, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    publish_range(&bus, "alice", 0..80).await;
    let unread = bus.unread.read().await;
    let writer = bus.clone();
    let task = tokio::spawn(async move {
        writer
            .update_settings_for_user("alice", 50, EventOverflowPolicy::DropNew)
            .await
    });
    for _ in 0..100 {
        tokio::task::yield_now().await;
    }
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(unread);
    assert_eq!(bus.settings_for_user("alice").await.max_records, 100);
    assert_eq!(bus.history.read().await["alice"].len(), 80);
    assert_eq!(bus.unread_count_for_user("alice").await, 80);
}

#[tokio::test]
async fn cancelled_replacement_never_splits_history_and_unread() {
    let bus = memory_bus();
    publish_range(&bus, "alice", 0..8).await;
    let unread = bus.unread.read().await;
    let writer = bus.clone();
    let task = tokio::spawn(async move {
        writer
            .replace_user_events("alice", vec![event("alice", 99)])
            .await;
    });
    for _ in 0..100 {
        tokio::task::yield_now().await;
    }
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(unread);
    assert_eq!(
        sequences(&bus.snapshot_for_user("alice").await),
        (0..8).collect::<Vec<_>>()
    );
    assert_eq!(bus.unread_count_for_user("alice").await, 8);
}

#[tokio::test]
async fn replacement_rejects_foreign_owner_records_without_modifying_either_owner() {
    let bus = memory_bus();
    publish_range(&bus, "alice", 0..2).await;
    publish_range(&bus, "bob", 10..12).await;
    bus.replace_user_events("alice", vec![event("alice", 98), event("bob", 99)])
        .await;
    assert_eq!(sequences(&bus.snapshot_for_user("alice").await), vec![0, 1]);
    assert_eq!(sequences(&bus.snapshot_for_user("bob").await), vec![10, 11]);
    assert_eq!(bus.unread_count_for_user("alice").await, 2);
}

#[tokio::test]
async fn full_persistence_queue_latches_bounded_memory_fallback_instead_of_spawning_writers() {
    let mut bus = memory_bus();
    bus.settings
        .write()
        .await
        .insert("alice".into(), UserEventSettings::default());
    bus.db_pool = Some(
        PgPoolOptions::new()
            .connect_lazy("postgres://fixture@127.0.0.1:1/unused")
            .unwrap(),
    );
    bus.db_disabled.store(false, Ordering::Relaxed);
    bus.schema_ready.set(()).unwrap();
    let (sender, receiver) = mpsc::channel(1);
    bus.persist_sender = sender;
    bus.publish(event("alice", 0)).await;
    bus.publish(event("alice", 1)).await;
    // No SQL connection is needed: the queue is deliberately owned but never consumed.
    assert!(
        bus.active_pool().is_none(),
        "overflow must not dispatch an unbounded bypass writer"
    );
    assert_eq!(receiver.len(), 1);
    assert_eq!(sequences(&bus.snapshot_for_user("alice").await), vec![0, 1]);
    assert_eq!(bus.unread_count_for_user("alice").await, 2);
}

#[tokio::test]
async fn cancelled_mutation_waiting_for_owner_admission_changes_nothing() {
    let bus = memory_bus();
    publish_range(&bus, "alice", 0..8).await;
    let guard = bus.mutation_admission("alice").await;
    let writer = bus.clone();
    let task = tokio::spawn(async move {
        writer.mark_all_read_for_user("alice").await;
    });
    tokio::task::yield_now().await;
    assert!(!task.is_finished());
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(guard);
    assert_eq!(bus.unread_count_for_user("alice").await, 8);
}

#[tokio::test]
async fn stalled_persistence_barrier_has_a_deadline_and_releases_owner_admission() {
    let mut bus = memory_bus();
    publish_range(&bus, "alice", 0..2).await;
    bus.db_pool = Some(
        PgPoolOptions::new()
            .connect_lazy("postgres://fixture@127.0.0.1:1/unused")
            .unwrap(),
    );
    bus.db_disabled.store(false, Ordering::Relaxed);
    let (sender, receiver) = mpsc::channel(1);
    bus.persist_sender = sender;
    tokio::time::timeout(
        StdDuration::from_secs(12),
        bus.mark_all_read_for_user("alice"),
    )
    .await
    .unwrap();
    assert!(bus.active_pool().is_none());
    assert_eq!(receiver.len(), 1);
    assert_eq!(bus.unread_count_for_user("alice").await, 0);
    bus.publish(event("alice", 2)).await;
    assert_eq!(bus.unread_count_for_user("alice").await, 1);
}

#[tokio::test]
async fn stalled_storage_operation_has_a_deadline_instead_of_holding_admission_forever() {
    let mut bus = memory_bus();
    bus.db_pool = Some(
        PgPoolOptions::new()
            .connect_lazy("postgres://fixture@127.0.0.1:1/unused")
            .unwrap(),
    );
    bus.db_disabled.store(false, Ordering::Relaxed);
    assert!(bus.active_pool().is_some());
    let error = bus
        .query_with_deadline(std::future::pending::<Result<(), sqlx::Error>>())
        .await
        .unwrap_err();
    assert!(matches!(error, sqlx::Error::PoolTimedOut));
    assert!(bus.db_disabled.load(Ordering::Relaxed));
}
