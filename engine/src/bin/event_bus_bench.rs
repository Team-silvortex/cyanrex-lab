//! Lightweight event-bus benchmark used for local hotspot triage.

use chrono::Utc;
use cyanrex_engine::models::event::{Event, EventCategory, EventColor, EventSeverity};
use cyanrex_engine::services::event_bus::{EventBus, EventOverflowPolicy};
use std::env;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
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

    let published_count = Arc::new(AtomicU64::new(0));
    let latency_total_nanos = Arc::new(AtomicU64::new(0));
    let latency_max_nanos = Arc::new(AtomicU64::new(0));
    let start = Instant::now();

    let mut handles = Vec::with_capacity(config.concurrency.max(1));
    for worker in 0..config.concurrency.max(1) {
        let bus = bus.clone();
        let usernames = usernames.clone();
        let payload = payload.clone();
        let published_count = published_count.clone();
        let latency_total_nanos = latency_total_nanos.clone();
        let latency_max_nanos = latency_max_nanos.clone();

        let handle = tokio::spawn(async move {
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
                published_count.fetch_add(1, Ordering::Relaxed);
                latency_total_nanos.fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
                let current = duration.as_nanos() as u64;
                loop {
                    let observed = latency_max_nanos.load(Ordering::Relaxed);
                    if current <= observed {
                        break;
                    }
                    if latency_max_nanos
                        .compare_exchange(observed, current, Ordering::Relaxed, Ordering::Relaxed)
                        .is_ok()
                    {
                        break;
                    }
                }

                seq += config.concurrency.max(1);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        if let Err(error) = handle.await {
            eprintln!("worker failed: {error}");
        }
    }

    let elapsed = start.elapsed();
    let total_published = published_count.load(Ordering::Relaxed);
    let throughput = if elapsed.is_zero() {
        0.0
    } else {
        total_published as f64 / elapsed.as_secs_f64()
    };
    let avg_latency_ms = if total_published == 0 {
        0.0
    } else {
        latency_total_nanos.load(Ordering::Relaxed) as f64 / total_published as f64 / 1_000_000.0
    };
    let max_latency_ms = latency_max_nanos.load(Ordering::Relaxed) as f64 / 1_000_000.0;

    println!("bench result");
    println!("  total events: {total_published}");
    println!("  elapsed: {:.3}s", elapsed.as_secs_f64());
    println!("  throughput: {throughput:.2} events/s");
    println!("  avg publish latency: {avg_latency_ms:.3} ms");
    println!("  max publish latency: {max_latency_ms:.3} ms");

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
