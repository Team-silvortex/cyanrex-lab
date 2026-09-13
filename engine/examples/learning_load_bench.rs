//! Read-only cold-process LearningStore benchmark. Fixtures are created by the paired runner.
use cyanrex_engine::services::learning_store::LearningStore;
use serde_json::json;
use std::{path::PathBuf, time::Duration};
use tokio::{
    sync::oneshot,
    time::{Instant, MissedTickBehavior},
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "learning_load_bench <fixture> <rows> <source-bytes>"
    );
    let path = PathBuf::from(&args[1]);
    let rows: usize = args[2].parse().unwrap();
    let source_bytes: usize = args[3].parse().unwrap();
    assert!((30..=50_000).contains(&rows));
    assert!((1..=8192).contains(&source_bytes));
    let file_bytes = std::fs::metadata(&path).unwrap().len();
    let store = LearningStore::with_local_data_path(path);
    let (ready_tx, ready_rx) = oneshot::channel();
    let (stop_tx, mut stop_rx) = oneshot::channel();
    let period = Duration::from_millis(2);
    let timer = tokio::spawn(async move {
        let mut interval = tokio::time::interval(period);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        interval.tick().await;
        ready_tx.send(()).unwrap();
        let (mut ticks, mut maximum) = (0usize, Duration::ZERO);
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                scheduled = interval.tick() => {
                    ticks += 1;
                    maximum = maximum.max(Instant::now().saturating_duration_since(scheduled));
                }
            }
        }
        (ticks, maximum)
    });
    ready_rx.await.unwrap();
    let start = Instant::now();
    let recent = store
        .recent_attempts_for_user("bench-user-0", 20)
        .await
        .unwrap();
    let first_read = start.elapsed();
    // Let an overdue tick run even when synchronous parsing blocked the executor until the query returned.
    tokio::time::sleep(period * 2).await;
    stop_tx.send(()).unwrap();
    let (ticks, maximum) = timer.await.unwrap();
    assert!(ticks > 0);
    assert_eq!(recent.len(), 20.min(rows.div_ceil(30)));
    assert!(recent
        .iter()
        .all(|row| row.username == "bench-user-0" && row.source.len() == source_bytes));
    let start = Instant::now();
    let warm = store
        .recent_attempts_for_user("bench-user-0", 20)
        .await
        .unwrap();
    let warm_read = start.elapsed();
    assert_eq!(recent[0].id, warm[0].id);
    let overview = store.teacher_overview().await.unwrap();
    assert_eq!(overview.active_students, 30);
    assert_eq!(
        overview
            .students
            .iter()
            .map(|s| s.total_attempts as usize)
            .sum::<usize>(),
        rows
    );
    println!(
        "{}",
        json!({
            "mode": "learning_cold_load", "runtime": "current_thread", "initial_rows": rows,
            "source_bytes": source_bytes, "file_bytes": file_bytes, "read_only": true,
            "first_read_ms": first_read.as_secs_f64() * 1000.0,
            "warm_read_ms": warm_read.as_secs_f64() * 1000.0,
            "timer_period_ms": period.as_millis(), "timer_ticks": ticks,
            "max_tick_delay_ms": maximum.as_secs_f64() * 1000.0,
        })
    );
}
