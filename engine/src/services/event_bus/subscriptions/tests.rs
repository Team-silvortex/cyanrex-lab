use super::*;
use crate::{
    models::event::{EventCategory, EventSeverity},
    services::event_bus::EventOverflowPolicy,
};
use serde_json::json;
use tokio::sync::broadcast::error::TryRecvError;

fn event(username: &str, seq: usize) -> Event {
    Event {
        username: username.into(),
        timestamp: chrono::DateTime::from_timestamp(1_700_000_000 + seq as i64, 0).unwrap(),
        source: "fanout-test".into(),
        event_type: "sequence".into(),
        category: EventCategory::Kernel,
        severity: EventSeverity::Success,
        color: EventSeverity::Success.color(),
        payload: json!({"seq": seq}),
    }
}

#[test]
fn matching_receivers_share_one_event_allocation() {
    let fanout = UserFanout::new(4);
    let mut first = fanout.subscribe("alice");
    let mut second = fanout.subscribe("alice");
    let mut other = fanout.subscribe("bob");
    assert_eq!(fanout.active_owner_count(), 2);
    fanout.publish(&event("alice", 1));
    let (first, second) = (first.try_recv().unwrap(), second.try_recv().unwrap());
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.username, "alice");
    assert!(matches!(other.try_recv(), Err(TryRecvError::Empty)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn json_is_lazy_and_encoded_once_across_concurrent_receivers() {
    let fanout = UserFanout::new(4);
    let mut receivers: Vec<_> = (0..16).map(|_| fanout.subscribe("alice")).collect();
    let mut input = event("alice", 1);
    input.payload = json!({"text": "局域网\n\"quoted\"", "repeat": [1, 1]});
    fanout.publish(&input);
    let first = receivers[0].recv().await.unwrap();
    assert!(
        first.serialized.get().is_none(),
        "publication must not eagerly encode JSON"
    );
    let expected = serde_json::to_string(&input).unwrap();
    let barrier = Arc::new(tokio::sync::Barrier::new(17));
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let (event, barrier, expected) = (first.clone(), barrier.clone(), expected.clone());
        tasks.spawn(async move {
            barrier.wait().await;
            let text = event.json().unwrap();
            assert_eq!(text, expected);
            text.as_ptr() as usize
        });
    }
    barrier.wait().await;
    let mut pointers = Vec::new();
    while let Some(result) = tasks.join_next().await {
        pointers.push(result.unwrap());
    }
    assert!(pointers.iter().all(|pointer| *pointer == pointers[0]));
    let second = receivers[1].recv().await.unwrap();
    assert_eq!(
        second.json().unwrap().as_ptr(),
        first.json().unwrap().as_ptr()
    );
}

#[test]
fn unrelated_traffic_cannot_lag_an_owner_but_own_slow_receivers_still_lag() {
    let fanout = UserFanout::new(2);
    let mut fast = fanout.subscribe("alice");
    let mut slow = fanout.subscribe("alice");
    let mut quiet = fanout.subscribe("bob");
    for seq in 0..16 {
        fanout.publish(&event("alice", seq));
        assert_eq!(fast.try_recv().unwrap().payload["seq"], seq);
    }
    assert!(matches!(slow.try_recv(), Err(TryRecvError::Lagged(14))));
    assert!(matches!(quiet.try_recv(), Err(TryRecvError::Empty)));
    fanout.publish(&event("bob", 16));
    assert_eq!(quiet.try_recv().unwrap().payload["seq"], 16);
    assert!(matches!(fast.try_recv(), Err(TryRecvError::Empty)));
}

#[test]
fn subscriptions_are_live_only_and_cleanup_after_the_last_connection() {
    let fanout = UserFanout::new(4);
    for seq in 0..1000 {
        fanout.publish(&event(&format!("owner-{seq}"), seq));
    }
    assert_eq!(
        fanout.active_owner_count(),
        0,
        "publishing must not grow the registry"
    );
    let first = fanout.subscribe("alice");
    let mut second = fanout.subscribe("alice");
    assert_eq!(fanout.active_owner_count(), 1);
    assert_eq!(fanout.receiver_count(), 2);
    drop(first);
    assert_eq!(fanout.active_owner_count(), 1);
    fanout.publish(&event("alice", 1));
    assert_eq!(second.try_recv().unwrap().payload["seq"], 1);
    drop(second);
    assert_eq!(fanout.active_owner_count(), 0);
    let mut next = fanout.subscribe("alice");
    assert!(
        matches!(next.try_recv(), Err(TryRecvError::Empty)),
        "no implicit replay"
    );
    drop(next);
    for seq in 0..1000 {
        drop(fanout.subscribe(&format!("owner-{seq}")));
    }
    assert_eq!(
        fanout.active_owner_count(),
        0,
        "churn must not retain idle entries"
    );
}

#[test]
fn stale_drop_cannot_remove_a_replacement_generation() {
    let fanout = UserFanout::new(4);
    let mut old = fanout.subscribe("alice");
    let last = fanout.subscribe("alice");
    // Model Drop being descheduled between releasing its receiver and locking the registry.
    drop(old.receiver.take());
    drop(last);
    assert_eq!(fanout.active_owner_count(), 0);
    let mut replacement = fanout.subscribe("alice");
    drop(replacement.receiver.take());
    drop(old);
    assert_eq!(
        fanout.active_owner_count(),
        1,
        "a stale guard cannot own new cleanup"
    );
    drop(replacement);
    assert_eq!(fanout.active_owner_count(), 0);
}

#[tokio::test]
async fn receivers_do_not_keep_the_registry_or_sender_alive() {
    let fanout = UserFanout::new(4);
    let clone = fanout.clone();
    let mut receiver = fanout.subscribe("alice");
    drop(fanout);
    clone.publish(&event("alice", 1));
    drop(clone);
    assert_eq!(receiver.recv().await.unwrap().payload["seq"], 1);
    assert!(matches!(receiver.recv().await, Err(RecvError::Closed)));
    assert!(receiver.registry.upgrade().is_none());
    assert!(receiver.channel.upgrade().is_none());
}

#[tokio::test]
async fn cancelling_recv_preserves_delivery_and_aborting_releases_the_subscription() {
    let fanout = UserFanout::new(4);
    let mut receiver = fanout.subscribe("alice");
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(1), receiver.recv())
            .await
            .is_err()
    );
    fanout.publish(&event("alice", 1));
    assert_eq!(receiver.recv().await.unwrap().payload["seq"], 1);
    let task = tokio::spawn(async move { receiver.recv().await });
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(fanout.receiver_count(), 0);
    assert_eq!(fanout.active_owner_count(), 0);
}

#[tokio::test]
async fn event_bus_keeps_global_compatibility_and_drop_new_semantics() {
    let bus = EventBus::new(128);
    bus.db_disabled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropNew)
        .await
        .unwrap();
    let mut alice = bus.subscribe_for_user("alice");
    let mut bob = bus.subscribe_for_user("bob");
    let mut global = bus.subscribe();
    for seq in 0..50 {
        bus.publish(event("alice", seq)).await;
        assert_eq!(alice.try_recv().unwrap().payload["seq"], seq);
        assert_eq!(global.try_recv().unwrap().payload["seq"], seq);
    }
    bus.mark_all_read_for_user("alice").await;
    bus.publish(event("alice", 50)).await;
    assert_eq!(bus.unread_count_for_user("alice").await, 0);
    assert_eq!(bus.snapshot_for_user("alice").await.len(), 50);
    assert!(matches!(alice.try_recv(), Err(TryRecvError::Empty)));
    assert!(matches!(global.try_recv(), Err(TryRecvError::Empty)));
    bus.publish(event("bob", 51)).await;
    assert_eq!(bob.try_recv().unwrap().username, "bob");
    assert_eq!(global.try_recv().unwrap().username, "bob");
    assert!(matches!(alice.try_recv(), Err(TryRecvError::Empty)));
    bus.update_settings_for_user("alice", 50, EventOverflowPolicy::DropOldest)
        .await
        .unwrap();
    bus.publish(event("alice", 52)).await;
    assert_eq!(alice.try_recv().unwrap().payload["seq"], 52);
    assert_eq!(global.try_recv().unwrap().payload["seq"], 52);
    assert_eq!(bus.unread_count_for_user("alice").await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_publishers_and_connection_churn_preserve_owner_and_writer_order() {
    let bus = Arc::new(EventBus::new(1024));
    bus.db_disabled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let mut anchors: Vec<_> = (0..4)
        .map(|user| {
            let owner = format!("user-{user}");
            (owner.clone(), bus.subscribe_for_user(&owner))
        })
        .collect();
    let barrier = Arc::new(tokio::sync::Barrier::new(17));
    let mut tasks = tokio::task::JoinSet::new();
    for worker in 0..8 {
        let (bus, barrier) = (bus.clone(), barrier.clone());
        tasks.spawn(async move {
            barrier.wait().await;
            for seq in 0..128 {
                let mut event = event(&format!("user-{}", worker % 4), seq);
                event.payload["worker"] = json!(worker);
                bus.publish(event).await;
            }
        });
    }
    for worker in 0..8 {
        let (bus, barrier) = (bus.clone(), barrier.clone());
        tasks.spawn(async move {
            barrier.wait().await;
            for seq in 0..128 {
                let existing = bus.subscribe_for_user(&format!("user-{}", worker % 4));
                let transient = bus.subscribe_for_user(&format!("transient-{worker}-{seq}"));
                tokio::task::yield_now().await;
                drop((existing, transient));
            }
        });
    }
    barrier.wait().await;
    while let Some(result) = tasks.join_next().await {
        result.unwrap();
    }
    assert_eq!(bus.user_fanout.active_owner_count(), 4);
    for (owner, receiver) in &mut anchors {
        let mut counts = [0; 8];
        for _ in 0..256 {
            let event = receiver.try_recv().unwrap();
            assert_eq!(&event.username, owner);
            let worker = event.payload["worker"].as_u64().unwrap() as usize;
            assert_eq!(event.payload["seq"], counts[worker]);
            counts[worker] += 1;
        }
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
        assert_eq!(counts.iter().filter(|&&count| count == 128).count(), 2);
    }
    drop(anchors);
    assert_eq!(bus.user_fanout.receiver_count(), 0);
    assert_eq!(bus.user_fanout.active_owner_count(), 0);
}
