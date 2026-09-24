use super::*;

#[tokio::test]
async fn authoritative_reads_distinguish_invalid_configuration_from_memory_only() {
    let memory = EventBus::with_database_url(16, None);
    assert!(memory
        .read_snapshot_for_user_filtered("alice", EventQueryFilters::default())
        .await
        .unwrap()
        .is_empty());
    assert_eq!(memory.read_unread_count_for_user("alice").await.unwrap(), 0);
    let invalid = EventBus::with_database_url(16, Some("not-a-postgresql-url".into()));
    invalid.publish(event("alice", 1)).await;
    assert_eq!(
        invalid
            .read_snapshot_for_user_filtered("alice", EventQueryFilters::default())
            .await
            .unwrap_err(),
        EventReadError::Unavailable
    );
    assert_eq!(
        invalid
            .read_unread_count_for_user("alice")
            .await
            .unwrap_err(),
        EventReadError::Unavailable
    );
    // Explicit legacy helpers retain their best-effort service contract, not the HTTP contract.
    assert_eq!(invalid.snapshot_for_user("alice").await.len(), 1);
}

#[tokio::test]
async fn authoritative_read_filters_are_checked_before_either_backend() {
    let bus = memory_bus();
    let unavailable = EventBus::with_database_url(16, Some("not-a-postgresql-url".into()));
    publish_range(&bus, "alice", 0..3).await;
    for filters in [
        EventQueryFilters {
            since_minutes: Some(i64::MAX),
            ..Default::default()
        },
        EventQueryFilters {
            since_minutes: Some(1_000_000_000_000),
            ..Default::default()
        },
        EventQueryFilters {
            since_minutes: Some(-1),
            ..Default::default()
        },
        EventQueryFilters {
            severity: Some("errror"),
            ..Default::default()
        },
        EventQueryFilters {
            category: Some(""),
            ..Default::default()
        },
        EventQueryFilters {
            start: Some(event("alice", 2).timestamp),
            end: Some(event("alice", 1).timestamp),
            ..Default::default()
        },
    ] {
        assert_eq!(
            bus.read_snapshot_for_user_filtered("alice", filters)
                .await
                .unwrap_err(),
            EventReadError::InvalidFilters
        );
        assert_eq!(
            unavailable
                .read_snapshot_for_user_filtered("alice", filters)
                .await
                .unwrap_err(),
            EventReadError::InvalidFilters
        );
    }
    let exact = EventQueryFilters {
        start: Some(event("alice", 1).timestamp),
        end: Some(event("alice", 1).timestamp),
        ..Default::default()
    };
    assert_eq!(
        sequences(
            &bus.read_snapshot_for_user_filtered("alice", exact)
                .await
                .unwrap()
        ),
        vec![1]
    );
    let relative = EventQueryFilters {
        since_minutes: Some(5),
        ..Default::default()
    }
    .checked_for_read()
    .unwrap();
    assert_eq!(relative.since_minutes, None);
    assert!(relative.start.is_some());
    assert_eq!(bus.unread_count_for_user("alice").await, 3);
}
