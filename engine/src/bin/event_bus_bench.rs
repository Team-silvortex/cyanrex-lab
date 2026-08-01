//! Lightweight event-bus benchmark used for local hotspot triage.

use chrono::Utc;
use cyanrex_engine::models::event::{Event, EventCategory, EventColor, EventSeverity};
use cyanrex_engine::services::event_bus::{EventBus, EventOverflowPolicy};
use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use serde_json::json;

#[derive(Clone)]
struct BenchConfig {
    total_events: usize,
    users: usize,
    concurrency: usize,
    payload_size: usize,
    max_records: usize,
    overflow_policy: EventOverflowPolicy,
    database_url: Option<String>,
    broadcast_buffer: usize,
    verify: bool,
    verify_timeout_secs: u64,
    output_json: bool,
    label: Option<String>,
}

impl BenchConfig {
    fn from_args() -> Self {
        let mut args = env::args().skip(1).collect::<Vec<_>>();
        if args.iter().any(|value| value == "--help" || value == "-h") {
            Self::print_help();
            std::process::exit(0);
        }

        Self {
            total_events: parse_arg(&mut args, "--events").unwrap_or(200_000),
            users: parse_arg(&mut args, "--users").unwrap_or(8),
            concurrency: parse_arg(&mut args, "--concurrency").unwrap_or(32),
            payload_size: parse_arg(&mut args, "--payload-size").unwrap_or(128),
            max_records: parse_arg(&mut args, "--max-records").unwrap_or(20_000),
            overflow_policy: parse_policy_arg(
                parse_str_arg(&mut args, "--policy").unwrap_or_else(|| "drop_oldest".to_string()),
            ),
            database_url: parse_optional_str_arg(&mut args, "--database-url"),
            broadcast_buffer: parse_arg(&mut args, "--broadcast-buffer").unwrap_or(1024),
            verify: parse_optional_bool_arg(&mut args, "--verify"),
            verify_timeout_secs: parse_arg(&mut args, "--verify-timeout").unwrap_or(30),
            output_json: parse_optional_bool_arg(&mut args, "--json"),
            label: parse_optional_str_arg(&mut args, "--label"),
        }
    }

    fn print_help() {
        println!(
            "Usage: cargo run --bin event_bus_bench -- [options]\n\
             \n\
             --events <number>          total publish count (default: 200000)\n\
             --users <number>           number of users (default: 8)\n\
             --concurrency <number>     publishing task count (default: 32)\n\
             --payload-size <number>    payload length in bytes (default: 128)\n\
                    --max-records <number>     per-user history cap (default: 20000)\n\
                    --policy <drop_oldest|drop_new>  overflow strategy (default: drop_oldest)\n\
                    --broadcast-buffer <number> channel size for in-memory event stream (default: 1024)\n\
                    --database-url <url>       override DATABASE_URL\n\
                    --label <text>             label for benchmark output\n\
                    --json                     emit JSON payload after text report\n\
                    --verify                   wait for persisted rows after publish\n\
                    --verify-timeout <secs>    seconds to wait in verify mode (default: 30)\n\
                    --help"
        );
    }
}

fn parse_arg<T: std::str::FromStr>(args: &mut Vec<String>, name: &str) -> Option<T> {
    let index = args.iter().position(|value| value == name)?;
    if index + 1 >= args.len() {
        return None;
    }
    let value = args.remove(index + 1);
    args.remove(index);
    value.parse::<T>().ok()
}

fn parse_optional_str_arg(args: &mut Vec<String>, name: &str) -> Option<String> {
    let index = args.iter().position(|value| value == name)?;
    if index + 1 >= args.len() {
        return None;
    }
    let value = args.remove(index + 1);
    args.remove(index);
    Some(value)
}

fn parse_str_arg(args: &mut Vec<String>, name: &str) -> Option<String> {
    parse_optional_str_arg(args, name)
}

fn parse_optional_bool_arg(args: &mut Vec<String>, name: &str) -> bool {
    args.iter()
        .position(|value| value == name)
        .is_some_and(|index| {
            args.remove(index);
            true
        })
}

fn parse_policy_arg(raw: String) -> EventOverflowPolicy {
    match raw.as_str() {
        "drop_new" => EventOverflowPolicy::DropNew,
        _ => EventOverflowPolicy::DropOldest,
    }
}

#[derive(Default)]
struct WorkerResult {
    published: u64,
    total_latency_nanos: u64,
    max_latency_nanos: u64,
    latencies: Vec<u64>,
}

fn percentile_ms(samples: &mut [u64], quantile: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    if quantile <= 0.0 {
        return samples[0] as f64 / 1_000_000.0;
    }
    if quantile >= 1.0 {
        return samples[samples.len() - 1] as f64 / 1_000_000.0;
    }

    samples.sort_unstable();
    let index = ((samples.len() as f64 - 1.0) * quantile).round() as usize;
    samples[index] as f64 / 1_000_000.0
}

#[tokio::main]
async fn main() {
    let config = BenchConfig::from_args();

    if let Some(database_url) = &config.database_url {
        println!("benchmark using DATABASE_URL={database_url}");
        std::env::set_var("DATABASE_URL", database_url);
    }

    let bus = Arc::new(EventBus::new(config.broadcast_buffer));

    let mut usernames = Vec::with_capacity(config.users.max(1));
    for index in 0..config.users.max(1) {
        let username = format!("bench-user-{index}");
        let _ = bus
            .update_settings_for_user(&username, config.max_records, config.overflow_policy)
            .await;
        usernames.push(username);
    }

    let mut expected_by_user = vec![0usize; usernames.len()];
    for index in 0..config.total_events {
        expected_by_user[index % usernames.len()] += 1;
    }

    let payload = "x".repeat(config.payload_size);

    let start = Instant::now();
    let concurrency = config.concurrency.max(1);

    let mut handles = Vec::with_capacity(concurrency);
    for worker in 0..concurrency {
        let bus = bus.clone();
        let usernames = usernames.clone();
        let payload = payload.clone();

        let handle = tokio::spawn(async move {
            let mut result = WorkerResult {
                latencies: Vec::with_capacity(config.total_events / concurrency + 1),
                ..WorkerResult::default()
            };
            let mut seq = worker;
            while seq < config.total_events {
                let username = &usernames[seq % usernames.len()];
                let event = Event {
                    username: username.clone(),
                    timestamp: Utc::now(),
                    source: "bench".to_string(),
                    event_type: "publish_bench".to_string(),
                    category: EventCategory::Platform,
                    severity: EventSeverity::Success,
                    color: EventColor::Green,
                    payload: json!({
                        "seq": seq as u64,
                        "worker": worker as u64,
                        "payload": &payload,
                    }),
                };

                let op_start = Instant::now();
                bus.publish(event).await;
                let duration = op_start.elapsed();
                let current = duration.as_nanos() as u64;
                result.published += 1;
                result.total_latency_nanos += current;
                if current > result.max_latency_nanos {
                    result.max_latency_nanos = current;
                }
                result.latencies.push(current);

                seq += config.concurrency.max(1);
            }
            result
        });
        handles.push(handle);
    }

    let mut total_published = 0u64;
    let mut total_latency_nanos = 0u64;
    let mut max_latency_nanos = 0u64;
    let mut all_latencies = Vec::with_capacity(config.total_events.max(1));
    for handle in handles {
        match handle.await {
            Ok(result) => {
                total_published += result.published;
                total_latency_nanos += result.total_latency_nanos;
                if result.max_latency_nanos > max_latency_nanos {
                    max_latency_nanos = result.max_latency_nanos;
                }
                all_latencies.extend(result.latencies);
            }
            Err(error) => {
                eprintln!("worker failed: {error}");
            }
        }
    }

    let elapsed = start.elapsed();
    let throughput = if elapsed.is_zero() {
        0.0
    } else {
        total_published as f64 / elapsed.as_secs_f64()
    };
    let avg_latency_ms = if total_published == 0 {
        0.0
    } else {
        total_latency_nanos as f64 / total_published as f64 / 1_000_000.0
    };
    let max_latency_ms = max_latency_nanos as f64 / 1_000_000.0;
    let p50_latency_ms = percentile_ms(&mut all_latencies, 0.50);
    let p95_latency_ms = percentile_ms(&mut all_latencies, 0.95);
    let p99_latency_ms = percentile_ms(&mut all_latencies, 0.99);

    println!("bench result");
    println!("  total events: {total_published}");
    println!("  elapsed: {:.3}s", elapsed.as_secs_f64());
    println!("  throughput: {throughput:.2} events/s");
    println!("  avg publish latency: {avg_latency_ms:.3} ms");
    println!("  max publish latency: {max_latency_ms:.3} ms");
    println!("  p50 publish latency: {p50_latency_ms:.3} ms");
    println!("  p95 publish latency: {p95_latency_ms:.3} ms");
    println!("  p99 publish latency: {p99_latency_ms:.3} ms");

    if config.output_json {
        let result = serde_json::json!({
            "label": config.label.unwrap_or_else(|| "default".to_string()),
            "total_events": total_published,
            "elapsed_ms": elapsed.as_millis(),
            "elapsed_seconds": elapsed.as_secs_f64(),
            "throughput": throughput,
            "avg_publish_latency_ms": avg_latency_ms,
            "max_publish_latency_ms": max_latency_ms,
            "p50_publish_latency_ms": p50_latency_ms,
            "p95_publish_latency_ms": p95_latency_ms,
            "p99_publish_latency_ms": p99_latency_ms,
            "users": config.users,
            "concurrency": config.concurrency,
            "payload_size": config.payload_size,
            "max_records": config.max_records,
            "verify": config.verify,
            "policy": match config.overflow_policy {
                EventOverflowPolicy::DropOldest => "drop_oldest",
                EventOverflowPolicy::DropNew => "drop_new",
            },
            "broadcast_buffer": config.broadcast_buffer,
        });
        println!("{}", result);
    }

    if config.verify {
        verify_persistence(
            bus,
            &usernames,
            &expected_by_user,
            config.max_records,
            Duration::from_secs(config.verify_timeout_secs),
        )
        .await;
    }
}

async fn verify_persistence(
    bus: Arc<EventBus>,
    usernames: &[String],
    expected_by_user: &[usize],
    max_records: usize,
    timeout: Duration,
) {
    for (index, username) in usernames.iter().enumerate() {
        let target = expected_by_user[index].min(max_records);

        let start = Instant::now();
        loop {
            let events = bus.snapshot_for_user(username).await;
            if events.len() >= target {
                println!(
                    "verify user {username}: observed {}, target {target}",
                    events.len()
                );
                break;
            }
            if start.elapsed() >= timeout {
                println!(
                    "verify user {username}: timeout after {:.1}s, observed {}, target {target}",
                    timeout.as_secs_f64(),
                    events.len()
                );
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }
}
