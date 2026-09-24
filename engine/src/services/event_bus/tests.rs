use super::*;
use crate::models::event::{EventCategory, EventSeverity};
use serde_json::json;
use tokio::sync::broadcast::error::TryRecvError;

mod deletion;
mod persistence;
mod retention;

fn memory_bus() -> EventBus {
    let bus = EventBus::new(1024);
    // Keep these tests on the memory path even if the caller configured a database.
    bus.db_disabled.store(true, Ordering::Relaxed);
    bus
}

fn event(username: &str, seq: usize) -> Event {
    let severity = if seq % 3 == 0 {
        EventSeverity::Success
    } else {
        EventSeverity::Warning
    };
    Event {
        username: username.to_owned(),
        timestamp: DateTime::from_timestamp(1_700_000_000 + seq as i64, 0).unwrap(),
        source: "history-test".into(),
        event_type: "sequence".into(),
        category: if seq % 2 == 0 {
            EventCategory::Kernel
        } else {
            EventCategory::Platform
        },
        severity,
        color: severity.color(),
        payload: json!({"seq": seq}),
    }
}

fn sequences(events: &[Event]) -> Vec<usize> {
    events
        .iter()
        .map(|event| event.payload["seq"].as_u64().unwrap() as usize)
        .collect()
}

async fn publish_range(bus: &EventBus, username: &str, range: std::ops::Range<usize>) {
    for seq in range {
        bus.publish(event(username, seq)).await;
    }
}

#[tokio::test]
async fn steady_state_eviction_does_not_grow_or_relocate_retained_events() {
    let bus = memory_bus();
    bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    bus.replace_user_events("alice", (0..50).map(|seq| event("alice", seq)).collect())
        .await;
    let (capacity, retained_address) = {
        let history = bus.history.read().await;
        let bucket = &history["alice"];
        (bucket.capacity(), &bucket[25] as *const Event as usize)
    };

    bus.publish(event("alice", 50)).await;

    let history = bus.history.read().await;
    let bucket = &history["alice"];
    assert_eq!(bucket.len(), 50);
    // White-box cost regression: no growth or survivor shifting on steady-state eviction.
    // This is not a public address-stability guarantee across resizing/replacement.
    assert_eq!(bucket.capacity(), capacity);
    assert_eq!(bucket[24].payload["seq"], 25);
    assert_eq!(&bucket[24] as *const Event as usize, retained_address);
}

#[tokio::test]
async fn drop_oldest_preserves_fifo_after_repeated_wraparound() {
    let bus = memory_bus();
    bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    let mut receiver = bus.subscribe();
    publish_range(&bus, "alice", 0..275).await;

    let snapshot = bus.snapshot_for_user("alice").await;
    assert_eq!(sequences(&snapshot), (225..275).collect::<Vec<_>>());
    assert_eq!(bus.unread_count_for_user("alice").await, 50);
    for seq in 0..275 {
        assert_eq!(receiver.try_recv().unwrap().payload["seq"], seq);
    }
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
}

#[tokio::test]
async fn drop_new_does_not_change_history_broadcast_or_unread_after_full() {
    let bus = memory_bus();
    bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropNew)
        .await
        .unwrap();
    publish_range(&bus, "alice", 0..50).await;
    bus.mark_all_read_for_user("alice").await;
    let mut receiver = bus.subscribe();
    publish_range(&bus, "alice", 50..100).await;

    assert_eq!(
        sequences(&bus.snapshot_for_user("alice").await),
        (0..50).collect::<Vec<_>>()
    );
    assert_eq!(bus.unread_count_for_user("alice").await, 0);
    assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));

    bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    bus.publish(event("alice", 100)).await;
    let mut expected: Vec<_> = (1..50).collect();
    expected.push(100);
    assert_eq!(sequences(&bus.snapshot_for_user("alice").await), expected);
    assert_eq!(bus.unread_count_for_user("alice").await, 1);
    assert_eq!(receiver.try_recv().unwrap().payload["seq"], 100);
}

#[tokio::test]
async fn shrinking_and_growing_a_wrapped_history_preserves_the_newest_events() {
    let bus = memory_bus();
    bus.update_settings_for_user("alice", 100, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    publish_range(&bus, "alice", 0..175).await;
    bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    assert_eq!(
        sequences(&bus.snapshot_for_user("alice").await),
        (125..175).collect::<Vec<_>>()
    );
    assert_eq!(bus.unread_count_for_user("alice").await, 50);

    bus.update_settings_for_user("alice", 75, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    publish_range(&bus, "alice", 175..220).await;
    assert_eq!(
        sequences(&bus.snapshot_for_user("alice").await),
        (145..220).collect::<Vec<_>>()
    );
    assert_eq!(bus.unread_count_for_user("alice").await, 75);
}

#[tokio::test]
async fn replacement_and_clear_remain_user_scoped_and_reset_unread() {
    let bus = memory_bus();
    bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    publish_range(&bus, "alice", 0..100).await;
    publish_range(&bus, "bob", 0..3).await;
    bus.replace_user_events("alice", (100..200).map(|seq| event("alice", seq)).collect())
        .await;
    assert_eq!(
        sequences(&bus.snapshot_for_user("alice").await),
        (150..200).collect::<Vec<_>>()
    );
    assert_eq!(bus.unread_count_for_user("alice").await, 0);
    bus.publish(event("alice", 200)).await;
    assert_eq!(
        sequences(&bus.snapshot_for_user("alice").await),
        (151..201).collect::<Vec<_>>()
    );
    assert_eq!(bus.unread_count_for_user("alice").await, 1);
    assert_eq!(
        bus.delete_user_events_filtered("alice", None, None, None, None, None)
            .await,
        50
    );
    assert!(bus.snapshot_for_user("alice").await.is_empty());
    assert_eq!(bus.unread_count_for_user("alice").await, 0);
    assert_eq!(
        sequences(&bus.snapshot_for_user("bob").await),
        vec![0, 1, 2]
    );
    assert_eq!(bus.unread_count_for_user("bob").await, 3);
    bus.publish(event("alice", 201)).await;
    assert_eq!(sequences(&bus.snapshot_for_user("alice").await), vec![201]);
}

#[tokio::test]
async fn filtered_snapshots_keep_latest_matching_events_in_fifo_order() {
    let bus = memory_bus();
    bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    publish_range(&bus, "alice", 0..150).await;
    let snapshot = bus
        .snapshot_for_user_filtered(
            "alice",
            EventQueryFilters {
                category: Some("kernel"),
                severity: Some("success"),
                limit: Some(3),
                start: Some(event("alice", 110).timestamp),
                end: Some(event("alice", 140).timestamp),
                ..EventQueryFilters::default()
            },
        )
        .await;
    assert_eq!(sequences(&snapshot), vec![126, 132, 138]);
    assert!(bus.snapshot_for_user("bob").await.is_empty());

    let mut detached = bus.snapshot_for_user("alice").await;
    detached[0].payload["seq"] = json!(999);
    assert_eq!(
        sequences(&bus.snapshot_for_user("alice").await),
        (100..150).collect::<Vec<_>>()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_users_retain_independent_ordered_histories() {
    let bus = memory_bus();
    let mut tasks = Vec::new();
    for user in ["alice", "bob", "carol", "dave"] {
        bus.update_settings_for_user(user, 50, EventOverflowPolicy::DropOldest)
            .await
            .unwrap();
        let bus = bus.clone();
        tasks.push(tokio::spawn(async move {
            publish_range(&bus, user, 0..500).await;
            let snapshot = bus.snapshot_for_user(user).await;
            assert!(snapshot.iter().all(|event| event.username == user));
            assert_eq!(sequences(&snapshot), (450..500).collect::<Vec<_>>());
            assert_eq!(bus.unread_count_for_user(user).await, 50);
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
}

#[tokio::test]
async fn limited_queries_handle_empty_zero_oversized_and_full_export_results() {
    let bus = memory_bus();
    let filters = EventQueryFilters {
        limit: Some(200),
        ..Default::default()
    };
    assert!(bus
        .snapshot_for_user_filtered("alice", filters)
        .await
        .is_empty());
    bus.update_settings_for_user("alice", 500, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    publish_range(&bus, "alice", 0..700).await;
    assert_eq!(
        sequences(&bus.snapshot_for_user_filtered("alice", filters).await),
        (500..700).collect::<Vec<_>>()
    );
    assert!(bus
        .snapshot_for_user_filtered(
            "alice",
            EventQueryFilters {
                limit: Some(0),
                ..filters
            }
        )
        .await
        .is_empty());
    let full = bus.snapshot_for_user("alice").await;
    assert_eq!(sequences(&full), (200..700).collect::<Vec<_>>());
    assert_eq!(
        sequences(
            &bus.snapshot_for_user_filtered(
                "alice",
                EventQueryFilters {
                    limit: Some(usize::MAX),
                    ..filters
                }
            )
            .await
        ),
        sequences(&full)
    );
    assert_eq!(bus.unread_count_for_user("alice").await, 500);
}

#[tokio::test]
async fn selected_result_storage_does_not_grow_past_the_known_output_bound() {
    let bus = memory_bus();
    bus.update_settings_for_user("alice", 5000, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    bus.replace_user_events("alice", (0..5000).map(|seq| event("alice", seq)).collect())
        .await;
    for limit in [Some(200), None, Some(0)] {
        let rows = bus
            .snapshot_for_user_filtered(
                "alice",
                EventQueryFilters {
                    limit,
                    ..Default::default()
                },
            )
            .await;
        let expected = limit.unwrap_or(5000);
        assert_eq!(rows.len(), expected);
        assert!(
            rows.capacity() <= expected,
            "selected output should not repeatedly grow its Event allocation: {} > {expected}",
            rows.capacity()
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bounded_reads_during_publication_remain_ordered_and_user_scoped() {
    let bus = memory_bus();
    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let mut tasks = Vec::new();
    for user in ["alice", "bob", "carol", "dave"] {
        bus.update_settings_for_user(user, 50, EventOverflowPolicy::DropOldest)
            .await
            .unwrap();
        let (writer, ready) = (bus.clone(), barrier.clone());
        tasks.push(tokio::spawn(async move {
            ready.wait().await;
            publish_range(&writer, user, 0..500).await;
        }));
        let (reader, ready) = (bus.clone(), barrier.clone());
        tasks.push(tokio::spawn(async move {
            ready.wait().await;
            for _ in 0..40 {
                let rows = reader
                    .snapshot_for_user_filtered(
                        user,
                        EventQueryFilters {
                            limit: Some(7),
                            ..Default::default()
                        },
                    )
                    .await;
                assert!(rows.len() <= 7);
                assert!(rows.iter().all(|event| event.username == user));
                assert!(sequences(&rows)
                    .windows(2)
                    .all(|pair| pair[0] + 1 == pair[1]));
                tokio::task::yield_now().await;
            }
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    for user in ["alice", "bob", "carol", "dave"] {
        let rows = bus
            .snapshot_for_user_filtered(
                user,
                EventQueryFilters {
                    limit: Some(7),
                    ..Default::default()
                },
            )
            .await;
        assert_eq!(sequences(&rows), (493..500).collect::<Vec<_>>());
    }
}
