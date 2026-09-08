//! Isolated service-level measurements; never loads eBPF or connects to a database.
use chrono::Utc;
use cyanrex_engine::{
    models::{
        event::{Event, EventCategory, EventColor, EventSeverity},
        learning::LabAttempt,
    },
    services::{
        ebpf_loader::EbpfLoader,
        event_bus::{EventBus, EventOverflowPolicy},
        learning_store::{LearningRunOutcome, LearningStore},
        runner_driver::{RunnerCheckRequest, RunnerCompletionRequest, RunnerOperationError},
        runner_manager::{RunnerConfig, RunnerManager},
    },
};
use serde_json::{json, Value};
use std::{
    hint::black_box,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::{broadcast::error::RecvError, Barrier};

fn summary(samples: &mut [u64]) -> Value {
    if samples.is_empty() {
        return json!({"count": 0});
    }
    samples.sort_unstable();
    let percentile = |q: f64| {
        samples[((samples.len() as f64 * q).ceil() as usize).saturating_sub(1)] as f64 / 1000.0
    };
    json!({"count": samples.len(), "p50_us": percentile(0.50), "p95_us": percentile(0.95),
        "p99_us": percentile(0.99), "max_us": samples[samples.len() - 1] as f64 / 1000.0,
        "mean_us": samples.iter().map(|v| *v as f64).sum::<f64>() / samples.len() as f64 / 1000.0})
}

fn number(args: &[String], index: usize, min: usize, max: usize) -> usize {
    let value = args
        .get(index)
        .expect("missing numeric argument")
        .parse()
        .expect("invalid integer");
    assert!(
        (min..=max).contains(&value),
        "argument {index} outside {min}..={max}"
    );
    value
}

fn event(username: &str, seq: usize, payload: &str) -> Event {
    Event {
        username: username.into(),
        timestamp: Utc::now(),
        source: "benchmark".into(),
        event_type: "synthetic".into(),
        category: EventCategory::Platform,
        severity: EventSeverity::Success,
        color: EventColor::Green,
        payload: json!({"seq": seq, "payload": payload}),
    }
}

async fn seed_bus(users: usize, records: usize, payload: &str) -> Arc<EventBus> {
    let bus = Arc::new(EventBus::new(1024));
    for user in 0..users {
        let name = format!("bench-user-{user}");
        let settings = bus
            .update_settings_for_user(&name, records, EventOverflowPolicy::DropOldest)
            .await
            .unwrap();
        assert_eq!(settings.max_records, records);
        bus.replace_user_events(
            &name,
            (0..records).map(|seq| event(&name, seq, payload)).collect(),
        )
        .await;
    }
    bus
}

async fn events(args: &[String]) -> Value {
    assert_eq!(
        args.len(),
        9,
        "events count users writers records payload subscribers readers"
    );
    let count = number(args, 2, 1, 2_000_000);
    let users = number(args, 3, 1, 64);
    let writers = number(args, 4, 1, 64);
    let records = number(args, 5, 50, 50_000);
    let size = number(args, 6, 0, 4096);
    let subscribers = number(args, 7, 0, 64);
    let readers = number(args, 8, 0, 8);
    let payload = "x".repeat(size);
    let bus = seed_bus(users, records, &payload).await;
    let barrier = Arc::new(Barrier::new(writers + readers + subscribers + 1));
    let done = Arc::new(AtomicBool::new(false));
    let mut consumers = Vec::new();
    for index in 0..subscribers {
        let mut receiver = bus.subscribe();
        let (barrier, done) = (barrier.clone(), done.clone());
        let name = format!("bench-user-{}", index % users);
        consumers.push(tokio::spawn(async move {
            let (mut received, mut matched, mut lagged, mut bytes) = (0u64, 0u64, 0u64, 0usize);
            barrier.wait().await;
            loop {
                match tokio::time::timeout(Duration::from_millis(10), receiver.recv()).await {
                    Ok(Ok(event)) => {
                        received += 1;
                        if event.username == name {
                            matched += 1;
                            bytes += black_box(serde_json::to_string(&event).unwrap()).len();
                        }
                    }
                    Ok(Err(RecvError::Lagged(count))) => lagged += count,
                    Ok(Err(RecvError::Closed)) => break,
                    Err(_) if done.load(Ordering::Acquire) => break,
                    Err(_) => {}
                }
            }
            (received, matched, lagged, bytes)
        }));
    }
    let mut pollers = Vec::new();
    for index in 0..readers {
        let (bus, barrier, done) = (bus.clone(), barrier.clone(), done.clone());
        let name = format!("bench-user-{}", index % users);
        pollers.push(tokio::spawn(async move {
            let mut samples = Vec::new();
            barrier.wait().await;
            while !done.load(Ordering::Acquire) {
                let start = Instant::now();
                black_box(bus.snapshot_for_user(&name).await);
                samples.push(start.elapsed().as_nanos() as u64);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            samples
        }));
    }
    let mut publishers = Vec::new();
    for worker in 0..writers {
        let (bus, barrier, payload) = (bus.clone(), barrier.clone(), payload.clone());
        publishers.push(tokio::spawn(async move {
            let mut samples = Vec::new();
            barrier.wait().await;
            for seq in (worker..count).step_by(writers) {
                let event = event(&format!("bench-user-{}", seq % users), seq, &payload);
                let start = Instant::now();
                bus.publish(event).await;
                samples.push(start.elapsed().as_nanos() as u64);
            }
            samples
        }));
    }
    let start = Instant::now();
    barrier.wait().await;
    let mut samples = Vec::with_capacity(count);
    for task in publishers {
        samples.extend(task.await.expect("publisher panicked"));
    }
    let elapsed = start.elapsed().as_secs_f64();
    done.store(true, Ordering::Release);
    assert_eq!(samples.len(), count);
    let (mut received, mut matched, mut lagged, mut bytes) = (0, 0, 0, 0);
    for consumer in consumers {
        let result = consumer.await.expect("subscriber panicked");
        received += result.0;
        matched += result.1;
        lagged += result.2;
        bytes += result.3;
    }
    assert_eq!(
        received + lagged,
        (count * subscribers) as u64,
        "must account for the entire broadcast stream"
    );
    let expected_matches: usize = (0..subscribers)
        .map(|i| count / users + usize::from(i % users < count % users))
        .sum();
    let mut read_samples = Vec::new();
    for poller in pollers {
        read_samples.extend(poller.await.expect("reader panicked"));
    }
    json!({"mode": "events", "events": count, "users": users, "writers": writers,
        "records": records, "payload_bytes": size, "subscribers": subscribers, "readers": readers,
        "prefilled": true, "elapsed_seconds": elapsed, "throughput": count as f64 / elapsed,
        "publish": summary(&mut samples), "snapshot": summary(&mut read_samples),
        "broadcast_received": received, "broadcast_lagged": lagged, "delivered": matched,
        "expected_deliveries": expected_matches, "serialized_bytes": bytes})
}

async fn snapshots(args: &[String]) -> Value {
    assert_eq!(args.len(), 5, "snapshots records payload repetitions");
    let records = number(args, 2, 50, 50_000);
    let size = number(args, 3, 0, 4096);
    let repetitions = number(args, 4, 1, 10_000);
    let bus = seed_bus(1, records, &"x".repeat(size)).await;
    let (mut reads, mut serialization) = (Vec::new(), Vec::new());
    for _ in 0..repetitions {
        let start = Instant::now();
        let rows = bus.snapshot_for_user("bench-user-0").await;
        reads.push(start.elapsed().as_nanos() as u64);
        assert_eq!(rows.len(), records);
        let start = Instant::now();
        black_box(serde_json::to_vec(&rows[rows.len().saturating_sub(100)..]).unwrap());
        serialization.push(start.elapsed().as_nanos() as u64);
    }
    json!({"mode": "snapshots", "records": records, "payload_bytes": size,
        "snapshot": summary(&mut reads), "serialize_last_100": summary(&mut serialization)})
}

const SOURCE: &str =
    "struct sample { int field; };\nint sample_run(struct sample *p) {\n  return p->field;\n}\n";

async fn compile(
    manager: &RunnerManager,
    kind: &str,
    source: &str,
) -> Result<bool, RunnerOperationError> {
    if kind.starts_with("complete") {
        let result = manager
            .complete(RunnerCompletionRequest {
                owner_username: "bench-compiler",
                code: source,
                line: 3,
                column: 13,
                selected_headers: &[],
            })
            .await?;
        assert!(
            result.response.ok,
            "completion failed: {}",
            result.response.message
        );
        Ok(result.cache_hit == Some(true))
    } else {
        let result = manager
            .check(RunnerCheckRequest {
                owner_username: "bench-compiler",
                code: source,
                selected_headers: &[],
            })
            .await?;
        assert!(
            result.response.ok,
            "check failed: {}",
            result.response.stderr
        );
        Ok(result.cache_hit == Some(true))
    }
}

async fn compiler(args: &[String]) -> Value {
    assert_eq!(args.len(), 5, "compiler check-cold|check-warm|complete-cold|complete-warm|check-burst|complete-burst repetitions width");
    let kind = &args[2];
    assert!([
        "check-cold",
        "check-warm",
        "complete-cold",
        "complete-warm",
        "check-burst",
        "complete-burst"
    ]
    .contains(&kind.as_str()));
    let repetitions = number(args, 3, 1, 50_000);
    let width = number(args, 4, 1, 64);
    let manager = RunnerManager::local(
        RunnerConfig {
            max_concurrent: 2,
            max_per_user: 1,
            execution_timeout: Duration::from_secs(45),
            instance_id: "benchmark".into(),
        },
        EbpfLoader::default(),
    );
    let warm = kind.ends_with("warm");
    if warm {
        assert!(!compile(&manager, kind, SOURCE).await.unwrap());
    }
    let (mut samples, mut rejected, mut hits) = (Vec::new(), 0usize, 0usize);
    let start = Instant::now();
    for index in 0..repetitions {
        let source = if warm {
            SOURCE.to_owned()
        } else {
            format!("{SOURCE}// edit {index}\n")
        };
        if kind.ends_with("burst") {
            let barrier = Arc::new(Barrier::new(width));
            let mut tasks = Vec::new();
            for _ in 0..width {
                let (manager, source, kind, barrier) = (
                    manager.clone(),
                    source.clone(),
                    kind.clone(),
                    barrier.clone(),
                );
                tasks.push(tokio::spawn(async move {
                    barrier.wait().await;
                    let start = Instant::now();
                    (
                        compile(&manager, &kind, &source).await,
                        start.elapsed().as_nanos() as u64,
                    )
                }));
            }
            for task in tasks {
                let (result, ns) = task.await.unwrap();
                match result {
                    Ok(hit) => {
                        samples.push(ns);
                        hits += usize::from(hit);
                    }
                    Err(RunnerOperationError::Busy) => rejected += 1,
                    Err(error) => panic!("unexpected compiler failure: {error}"),
                }
            }
        } else {
            assert_eq!(width, 1);
            let start = Instant::now();
            hits += usize::from(compile(&manager, kind, &source).await.unwrap());
            samples.push(start.elapsed().as_nanos() as u64);
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    json!({"mode": "compiler", "kind": kind, "attempts": repetitions * width,
        "successful": samples.len(), "rejected_busy": rejected, "cache_hits": hits,
        "elapsed_seconds": elapsed, "successful_throughput": samples.len() as f64 / elapsed,
        "accepted_latency": summary(&mut samples)})
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("cleanup benchmark-owned scratch directory");
    }
}

async fn learning(args: &[String]) -> Value {
    assert_eq!(
        args.len(),
        6,
        "learning recent|overview|record rows source_bytes repetitions"
    );
    let kind = &args[2];
    assert!(["recent", "overview", "record"].contains(&kind.as_str()));
    let rows = number(args, 3, 30, 50_000);
    let size = number(args, 4, 1, 4096);
    let repetitions = number(args, 5, 1, 1000);
    let root =
        std::env::temp_dir().join(format!("cyanrex-bench-learning-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let scratch = Scratch(root);
    let path = scratch.0.join("attempts.json");
    let source = format!("{SOURCE}//{}", "x".repeat(size));
    let attempts: Vec<_> = (0..rows)
        .map(|index| LabAttempt {
            id: format!("bench-attempt-{index}"),
            username: format!("bench-user-{}", index % 30),
            lab_id: "01-first-program".into(),
            template_id: Some("xdp-pass".into()),
            source: source.clone(),
            source_sha256: "synthetic-fixture".into(),
            run_success: false,
            stage: "compile".into(),
            attach_expected: false,
            attach_verified: false,
            completed: false,
            feedback: vec![],
            teacher_feedback: None,
            created_at: Utc::now() + chrono::Duration::milliseconds(index as i64),
        })
        .collect();
    std::fs::write(&path, serde_json::to_vec(&attempts).unwrap()).unwrap();
    drop(attempts);
    let store = LearningStore::with_local_data_path(path.clone());
    assert!(!store.attempts_for_user("bench-user-0").await.is_empty());
    let mut samples = Vec::new();
    for _ in 0..repetitions {
        let start = Instant::now();
        match kind.as_str() {
            "recent" => {
                black_box(store.recent_attempts_for_user("bench-user-0", 20).await);
            }
            "overview" => {
                assert_eq!(
                    black_box(store.teacher_overview().await).active_students,
                    30
                );
            }
            "record" => {
                store
                    .record_run(
                        "bench-user-0",
                        LearningRunOutcome {
                            lab_id: "01-first-program",
                            template_id: Some("xdp-pass"),
                            source: &source,
                            run_success: false,
                            stage: "compile",
                            attach_expected: false,
                            attach_verified: false,
                        },
                    )
                    .await
                    .unwrap();
            }
            _ => unreachable!(),
        }
        samples.push(start.elapsed().as_nanos() as u64);
    }
    json!({"mode": "learning", "kind": kind, "initial_rows": rows, "source_bytes": source.len(),
        "students": 30, "storage": "isolated_local_file", "file_bytes": std::fs::metadata(&path).unwrap().len(),
        "latency": summary(&mut samples)})
}

#[tokio::main]
async fn main() {
    assert!(
        std::env::var("DATABASE_URL").unwrap_or_default().is_empty(),
        "unset DATABASE_URL before benchmarking"
    );
    let args: Vec<_> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("help");
    let result = match mode {
        "events" => events(&args).await,
        "snapshots" => snapshots(&args).await,
        "compiler" => compiler(&args).await,
        "learning" => learning(&args).await,
        _ => {
            eprintln!("Usage: mainline_bench events|snapshots|compiler|learning <mode-specific positional arguments>");
            std::process::exit(2);
        }
    };
    println!("{result}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_summary_sorts_once_and_uses_nearest_rank() {
        let mut samples = vec![30_000, 10_000, 20_000, 50_000, 40_000];
        let value = summary(&mut samples);
        assert_eq!(value["count"], 5);
        assert_eq!(value["p50_us"], 30.0);
        assert_eq!(value["p95_us"], 50.0);
        assert_eq!(value["max_us"], 50.0);
        assert_eq!(value["mean_us"], 30.0);
    }

    #[test]
    fn empty_samples_are_explicit_and_not_fake_zero_latency() {
        let value = summary(&mut []);
        assert_eq!(value["count"], 0);
        assert!(value["p50_us"].is_null());
    }
}
