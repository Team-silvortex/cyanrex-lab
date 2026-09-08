//! Manual service benchmark, excluded from normal test runs. No public API seam is needed.
use super::*;
use crate::models::event::{EventCategory, EventSeverity};
use serde_json::{json, Value};
use std::hint::black_box;
use tokio::sync::Barrier;

fn event(username: &str, seq: usize) -> Event {
    Event {
        username: username.into(),
        timestamp: Utc::now(),
        source: "read-benchmark".into(),
        event_type: "synthetic".into(),
        category: if seq % 20 == 0 {
            EventCategory::Kernel
        } else {
            EventCategory::Platform
        },
        severity: EventSeverity::Success,
        color: EventSeverity::Success.color(),
        payload: json!({"seq": seq, "payload": "x".repeat(128)}),
    }
}

fn summary(samples: &mut [u64]) -> Value {
    if samples.is_empty() {
        return json!({"count": 0});
    }
    samples.sort_unstable();
    let percentile = |q: f64| {
        samples[((samples.len() as f64 * q).ceil() as usize).saturating_sub(1)] as f64 / 1000.0
    };
    json!({"count": samples.len(), "p50_us": percentile(0.50),
        "p95_us": percentile(0.95), "p99_us": percentile(0.99)})
}

async fn seed(users: usize, records: usize) -> Arc<EventBus> {
    assert!(std::env::var("DATABASE_URL").unwrap_or_default().is_empty());
    let bus = Arc::new(EventBus::new(1024));
    for user in 0..users {
        let name = format!("bench-{user}");
        bus.update_settings_for_user(&name, records, EventOverflowPolicy::DropOldest)
            .await
            .unwrap();
        bus.replace_user_events(&name, (0..records).map(|seq| event(&name, seq)).collect())
            .await;
    }
    bus
}

async fn snapshots(records: usize, kind: &str) -> Value {
    let bus = seed(1, records).await;
    let (filters, expected) = match kind {
        "latest" => (
            EventQueryFilters {
                limit: Some(200),
                ..Default::default()
            },
            200,
        ),
        "filtered" => (
            EventQueryFilters {
                category: Some("kernel"),
                limit: Some(200),
                ..Default::default()
            },
            (records / 20).min(200),
        ),
        "miss" => (
            EventQueryFilters {
                severity: Some("error"),
                limit: Some(200),
                ..Default::default()
            },
            0,
        ),
        "full" => (EventQueryFilters::default(), records),
        _ => panic!("unknown snapshot kind"),
    };
    let mut samples = Vec::new();
    for _ in 0..200 {
        let start = Instant::now();
        let rows = bus.snapshot_for_user_filtered("bench-0", filters).await;
        samples.push(start.elapsed().as_nanos() as u64);
        assert_eq!(black_box(&rows).len(), expected);
    }
    json!({"mode": kind, "records": records, "returned": expected, "snapshot": summary(&mut samples)})
}

async fn mixed(records: usize) -> Value {
    let bus = seed(8, records).await;
    let barrier = Arc::new(Barrier::new(32 + 8 + 1));
    let done = Arc::new(AtomicBool::new(false));
    let mut publishers = Vec::new();
    for worker in 0..32 {
        let (bus, barrier, done) = (bus.clone(), barrier.clone(), done.clone());
        publishers.push(tokio::spawn(async move {
            let name = format!("bench-{}", worker % 8);
            let mut seq = worker;
            let mut samples = Vec::new();
            barrier.wait().await;
            while !done.load(Ordering::Acquire) {
                let event = event(&name, seq);
                let start = Instant::now();
                bus.publish(event).await;
                samples.push(start.elapsed().as_nanos() as u64);
                seq += 32;
            }
            samples
        }));
    }
    let mut readers = Vec::new();
    for user in 0..8 {
        let (bus, barrier, done) = (bus.clone(), barrier.clone(), done.clone());
        readers.push(tokio::spawn(async move {
            let name = format!("bench-{user}");
            let mut samples = Vec::new();
            barrier.wait().await;
            while !done.load(Ordering::Acquire) {
                let start = Instant::now();
                let rows = bus
                    .snapshot_for_user_filtered(
                        &name,
                        EventQueryFilters {
                            limit: Some(200),
                            ..Default::default()
                        },
                    )
                    .await;
                samples.push(start.elapsed().as_nanos() as u64);
                assert_eq!(black_box(&rows).len(), 200);
                drop(rows);
                tokio::time::sleep(StdDuration::from_millis(10)).await;
            }
            samples
        }));
    }
    let start = Instant::now();
    barrier.wait().await;
    tokio::time::sleep(StdDuration::from_secs(2)).await;
    done.store(true, Ordering::Release);
    let mut publishes = Vec::new();
    for publisher in publishers {
        publishes.extend(publisher.await.unwrap());
    }
    let elapsed = start.elapsed().as_secs_f64();
    let mut reads = Vec::new();
    for reader in readers {
        reads.extend(reader.await.unwrap());
    }
    json!({"mode": "mixed", "records": records, "users": 8, "writers": 32,
        "readers": 8, "limit": 200, "read_sleep_ms": 10, "elapsed_seconds": elapsed,
        "throughput": publishes.len() as f64 / elapsed,
        "publish": summary(&mut publishes), "snapshot": summary(&mut reads)})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
#[ignore = "manual release benchmark; use scripts/bench-event-history-reads.mjs"]
async fn measure() {
    let kind = std::env::var("CYANREX_READ_BENCH_KIND").expect("missing benchmark kind");
    let records: usize = std::env::var("CYANREX_READ_BENCH_RECORDS")
        .expect("missing history capacity")
        .parse()
        .unwrap();
    assert!([500, 5000, 20000, 50000].contains(&records));
    let result = if kind == "mixed" {
        mixed(records).await
    } else {
        snapshots(records, &kind).await
    };
    println!("CYANREX_READ_BENCH={result}");
}
