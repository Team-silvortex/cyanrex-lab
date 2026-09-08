//! Private A/B harness: both delivery paths use the current EventBus and identical matched work.
use super::*;
use crate::{
    models::event::{EventCategory, EventSeverity},
    services::event_bus::EventOverflowPolicy,
};
use axum::extract::ws::Utf8Bytes;
use serde_json::{json, Value};
use std::{hint::black_box, time::Instant};
use tokio::sync::Barrier;

const COUNT: usize = 240_000;
const SUBSCRIBERS: usize = 32;
const WRITERS: usize = 32;
// Deliberately larger than the whole run: throughput must not be purchased with missing records.
// This is not production channel sizing; tiny/default capacity transport checks are separate.
const CAPACITY: usize = 262_144;

enum Consumer {
    Global(broadcast::Receiver<Event>),
    Owner(UserEventSubscription),
}

impl Consumer {
    async fn next(&mut self, owner: &str) -> (usize, usize) {
        fn inspect(event: &Event, owner: &str) -> (usize, usize) {
            if event.username == owner {
                (
                    1,
                    black_box(Utf8Bytes::from(serde_json::to_string(event).unwrap())).len(),
                )
            } else {
                (0, 0)
            }
        }
        match self {
            Self::Global(receiver) => {
                inspect(&receiver.recv().await.expect("no lag allowed"), owner)
            }
            Self::Owner(receiver) => {
                let event = receiver.recv().await.expect("no lag allowed");
                assert_eq!(event.username, owner, "owner queue must never leak events");
                // Match the real handler, including a separate frame payload copy per socket.
                (1, black_box(Utf8Bytes::from(event.json().unwrap())).len())
            }
        }
    }
}

fn event(owner: &str, seq: usize, payload: &str) -> Event {
    Event {
        username: owner.into(),
        timestamp: chrono::DateTime::from_timestamp(1_700_000_000 + seq as i64, 0).unwrap(),
        source: "fanout-bench".into(),
        event_type: "sequence".into(),
        category: EventCategory::Kernel,
        severity: EventSeverity::Success,
        color: EventSeverity::Success.color(),
        payload: json!({"seq": seq, "data": payload}),
    }
}

fn summary(samples: &mut [u64]) -> Value {
    samples.sort_unstable();
    let percentile = |p: f64| samples[((samples.len() - 1) as f64 * p) as usize] as f64 / 1000.0;
    json!({"count": samples.len(), "p50_us": percentile(0.5), "p95_us": percentile(0.95),
        "p99_us": percentile(0.99)})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
#[ignore = "manual release benchmark; use scripts/bench-event-fanout.mjs"]
async fn measure() {
    assert!(std::env::var("DATABASE_URL")
        .unwrap_or_default()
        .trim()
        .is_empty());
    let mode = std::env::var("CYANREX_FANOUT_MODE").expect("mode required");
    assert!(matches!(mode.as_str(), "global" | "owner"));
    let users: usize = std::env::var("CYANREX_FANOUT_USERS")
        .unwrap()
        .parse()
        .unwrap();
    let size: usize = std::env::var("CYANREX_FANOUT_PAYLOAD")
        .unwrap()
        .parse()
        .unwrap();
    assert!(matches!(users, 1 | 8));
    assert!(matches!(size, 128 | 4096));
    let payload = "x".repeat(size);
    let bus = Arc::new(EventBus::new(CAPACITY));
    for user in 0..users {
        let owner = format!("bench-{user}");
        bus.update_settings_for_user(&owner, 500, EventOverflowPolicy::DropOldest)
            .await
            .unwrap();
        bus.replace_user_events(
            &owner,
            (0..500).map(|seq| event(&owner, seq, &payload)).collect(),
        )
        .await;
    }
    let barrier = Arc::new(Barrier::new(WRITERS + SUBSCRIBERS + 1));
    let mut consumers = Vec::new();
    for subscriber in 0..SUBSCRIBERS {
        let owner = format!("bench-{}", subscriber % users);
        let (mut receiver, target) = if mode == "global" {
            (Consumer::Global(bus.subscribe()), COUNT)
        } else {
            (
                Consumer::Owner(bus.subscribe_for_user(&owner)),
                COUNT / users,
            )
        };
        let barrier = barrier.clone();
        consumers.push(tokio::spawn(async move {
            let (mut matched, mut bytes) = (0, 0);
            barrier.wait().await;
            for _ in 0..target {
                let result = receiver.next(&owner).await;
                matched += result.0;
                bytes += result.1;
            }
            assert_eq!(matched, COUNT / users);
            (target, matched, bytes)
        }));
    }
    let mut writers = Vec::new();
    for worker in 0..WRITERS {
        let (bus, barrier, payload) = (bus.clone(), barrier.clone(), payload.clone());
        writers.push(tokio::spawn(async move {
            let mut samples = Vec::with_capacity(COUNT / WRITERS);
            barrier.wait().await;
            for seq in (worker..COUNT).step_by(WRITERS) {
                let event = event(&format!("bench-{}", seq % users), seq, &payload);
                let start = Instant::now();
                bus.publish(event).await;
                samples.push(start.elapsed().as_nanos() as u64);
            }
            samples
        }));
    }
    let start = Instant::now();
    barrier.wait().await;
    let mut samples = Vec::with_capacity(COUNT);
    for writer in writers {
        samples.extend(writer.await.unwrap());
    }
    let publish_seconds = start.elapsed().as_secs_f64();
    let (mut received, mut matched, mut bytes) = (0, 0, 0);
    for consumer in consumers {
        let row = consumer.await.unwrap();
        received += row.0;
        matched += row.1;
        bytes += row.2;
    }
    let complete_seconds = start.elapsed().as_secs_f64();
    assert_eq!(samples.len(), COUNT);
    assert_eq!(matched, COUNT * SUBSCRIBERS / users);
    assert_eq!(bus.user_fanout.active_owner_count(), 0);
    assert_eq!(bus.sender.receiver_count(), 0);
    let result = json!({"mode": mode, "events": COUNT, "users": users, "writers": WRITERS,
        "subscribers": SUBSCRIBERS, "payload_bytes": size, "history_records_per_owner": 500,
        "channel_capacity": CAPACITY, "publish_seconds": publish_seconds,
        "complete_seconds": complete_seconds, "throughput": COUNT as f64 / complete_seconds,
        "publish": summary(&mut samples), "received_copies": received, "matched_copies": matched,
        "expected_matching_copies": COUNT * SUBSCRIBERS / users, "lagged_copies": 0,
        "serialized_bytes": bytes, "remaining_owner_queues": bus.user_fanout.active_owner_count()});
    println!("CYANREX_FANOUT_BENCH={result}");
}
