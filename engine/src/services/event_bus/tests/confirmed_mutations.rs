use super::*;

#[tokio::test]
async fn confirmed_mutations_distinguish_bad_configuration_and_validate_before_storage() {
    let bus = EventBus::with_database_url(16, Some("invalid-database-url".into()));
    publish_range(&bus, "alice", 0..4).await;
    assert_eq!(
        bus.acknowledge_all_read_for_user("alice").await,
        Err(EventMutationError::Unconfirmed)
    );
    assert_eq!(
        bus.delete_user_events_confirmed("alice", EventQueryFilters::default())
            .await,
        Err(EventMutationError::Unconfirmed)
    );
    for filters in [
        EventQueryFilters {
            category: Some("typo"),
            ..Default::default()
        },
        EventQueryFilters {
            severity: Some(""),
            ..Default::default()
        },
        EventQueryFilters {
            since_minutes: Some(i64::MAX),
            ..Default::default()
        },
        EventQueryFilters {
            start: Some(event("alice", 2).timestamp),
            end: Some(event("alice", 1).timestamp),
            ..Default::default()
        },
    ] {
        assert_eq!(
            bus.delete_user_events_confirmed("alice", filters).await,
            Err(EventMutationError::InvalidFilters)
        );
    }
    assert_eq!(bus.history.read().await["alice"].len(), 4);
    assert_eq!(bus.unread.read().await["alice"], 4);
}

#[tokio::test]
async fn confirmed_memory_mutation_cancellation_does_not_split_cache_publication() {
    let bus = memory_bus();
    publish_range(&bus, "alice", 0..4).await;
    let unread = bus.unread.write().await;
    assert!(tokio::time::timeout(
        StdDuration::from_millis(50),
        bus.acknowledge_all_read_for_user("alice")
    )
    .await
    .is_err());
    assert!(tokio::time::timeout(
        StdDuration::from_millis(50),
        bus.delete_user_events_confirmed("alice", EventQueryFilters::default())
    )
    .await
    .is_err());
    drop(unread);
    bus.drop_new_count_cache
        .write()
        .await
        .insert("alice".into(), (4, Instant::now()));
    let mut admitted = bus.drop_new_admitted.write().await;
    admitted.insert("alice".into(), 4);
    assert!(tokio::time::timeout(
        StdDuration::from_millis(50),
        bus.delete_user_events_confirmed("alice", EventQueryFilters::default())
    )
    .await
    .is_err());
    assert_eq!(admitted["alice"], 4);
    drop(admitted);
    assert_eq!(bus.drop_new_count_cache.read().await["alice"].0, 4);
    assert_eq!(bus.history.read().await["alice"].len(), 4);
    assert_eq!(bus.unread.read().await["alice"], 4);
    let deleted = bus
        .delete_user_events_confirmed(
            "alice",
            EventQueryFilters {
                category: Some("kernel"),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(deleted, 2);
    assert_eq!(bus.unread.read().await["alice"], 2);
    assert!(!bus.drop_new_admitted.read().await.contains_key("alice"));
    assert!(!bus.drop_new_count_cache.read().await.contains_key("alice"));
}

#[tokio::test]
async fn confirmed_mutation_admission_deadline_does_not_dispatch_later() {
    let bus = memory_bus();
    publish_range(&bus, "alice", 0..4).await;
    let admission = bus.mutation_admission("alice").await;
    let (marked, deleted) = tokio::time::timeout(StdDuration::from_secs(12), async {
        tokio::join!(
            bus.acknowledge_all_read_for_user("alice"),
            bus.delete_user_events_confirmed("alice", EventQueryFilters::default())
        )
    })
    .await
    .unwrap();
    assert_eq!(marked, Err(EventMutationError::Unconfirmed));
    assert_eq!(deleted, Err(EventMutationError::Unconfirmed));
    drop(admission);
    let _admission = bus.mutation_admission("alice").await;
    assert_eq!(bus.history.read().await["alice"].len(), 4);
    assert_eq!(bus.unread.read().await["alice"], 4);
}
