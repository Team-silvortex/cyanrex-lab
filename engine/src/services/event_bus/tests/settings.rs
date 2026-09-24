use super::*;

#[tokio::test]
async fn confirmed_settings_distinguish_invalid_storage_and_intentional_memory() {
    let invalid = EventBus::with_database_url(16, Some("invalid-database-url".into()));
    publish_range(&invalid, "alice", 0..80).await;
    assert_eq!(
        invalid.read_settings_for_user("alice").await.unwrap_err(),
        EventReadError::Unavailable
    );
    assert_eq!(
        invalid
            .update_settings_confirmed("alice", 50, EventOverflowPolicy::DropNew)
            .await
            .unwrap_err(),
        EventMutationError::Unconfirmed
    );
    assert_eq!(invalid.history.read().await["alice"].len(), 80);
    assert_eq!(invalid.unread.read().await["alice"], 80);
    assert_eq!(invalid.settings.read().await["alice"].max_records, 500);
    let bus = memory_bus();
    assert_eq!(
        bus.read_settings_for_user("alice")
            .await
            .unwrap()
            .max_records,
        500
    );
    publish_range(&bus, "alice", 0..80).await;
    let saved = bus
        .update_settings_confirmed("alice", 0, EventOverflowPolicy::DropNew)
        .await
        .unwrap();
    assert_eq!(saved.max_records, 50);
    bus.publish(event("alice", 80)).await;
    assert_eq!(
        bus.read_settings_for_user("alice")
            .await
            .unwrap()
            .overflow_policy,
        EventOverflowPolicy::DropNew
    );
    assert_eq!(
        sequences(&bus.snapshot_for_user("alice").await),
        (30..80).collect::<Vec<_>>()
    );
    assert_eq!(bus.unread.read().await["alice"], 50);
}

#[tokio::test]
async fn confirmed_settings_cancelled_memory_publication_and_admission_leave_state_intact() {
    let bus = memory_bus();
    publish_range(&bus, "alice", 0..80).await;
    bus.drop_new_count_cache
        .write()
        .await
        .insert("alice".into(), (80, Instant::now()));
    let mut last_lock = bus.drop_new_admitted.write().await;
    last_lock.insert("alice".into(), 80);
    assert!(tokio::time::timeout(
        StdDuration::from_millis(50),
        bus.update_settings_confirmed("alice", 50, EventOverflowPolicy::DropNew)
    )
    .await
    .is_err());
    assert_eq!(last_lock["alice"], 80);
    drop(last_lock);
    let admission = bus.mutation_admission("alice").await;
    assert_eq!(
        tokio::time::timeout(
            StdDuration::from_secs(12),
            bus.update_settings_confirmed("alice", 50, EventOverflowPolicy::DropNew)
        )
        .await
        .unwrap()
        .unwrap_err(),
        EventMutationError::Unconfirmed
    );
    drop(admission);
    let _admission = bus.mutation_admission("alice").await;
    assert_eq!(bus.settings.read().await["alice"].max_records, 500);
    assert_eq!(bus.history.read().await["alice"].len(), 80);
    assert_eq!(bus.unread.read().await["alice"], 80);
    assert_eq!(bus.drop_new_count_cache.read().await["alice"].0, 80);
    assert_eq!(bus.drop_new_admitted.read().await["alice"], 80);
}
