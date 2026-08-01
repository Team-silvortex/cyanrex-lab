use chrono::{DateTime, Duration, Utc};
use sqlx::{types::Json, PgPool, Postgres, QueryBuilder, Row};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration as TokioDuration};

use super::event_bus::{EventBus, EventOverflowPolicy, DB_PERSIST_QUEUE_CAPACITY};
use super::event_bus_codec::{to_category_str, to_color_str, to_severity_str};
use super::event_bus_db_config::PersistQueuePressureConfig;
use crate::models::event::Event;
use std::{collections::HashMap, time::Instant as StdInstant};
use tracing::warn;

const DB_PERSIST_QUERY_LIMIT: usize = 5000;
const DB_PERSIST_BATCH_SIZE: usize = 64;
const DB_PERSIST_BATCH_WAIT_MS: u64 = 5;

#[derive(Debug, Clone)]
pub(crate) struct PersistRequest {
    pub(crate) event: Event,
    pub(crate) max_records: usize,
    pub(crate) overflow_policy: EventOverflowPolicy,
}

pub(crate) fn new_persist_request(
    event: Event,
    max_records: usize,
    overflow_policy: EventOverflowPolicy,
) -> PersistRequest {
    PersistRequest {
        event,
        max_records,
        overflow_policy,
    }
}

pub(crate) async fn run_persist_loop(
    bus: EventBus,
    pool: PgPool,
    mut receiver: mpsc::Receiver<PersistRequest>,
) {
    let mut channel_closed = false;
    let mut max_pending_requests = 0usize;
    let mut queue_pressure = false;
    let mut last_warning_time: Option<StdInstant> = None;
    let pressure_cfg = PersistQueuePressureConfig::from_env();
    let warn_threshold = pressure_cfg.warning_threshold(DB_PERSIST_QUEUE_CAPACITY);
    let clear_threshold = pressure_cfg.recover_threshold(DB_PERSIST_QUEUE_CAPACITY);

    while let Some(first_request) = receiver.recv().await {
        if bus.active_pool().is_none() {
            continue;
        }
        if bus.ensure_schema().await.is_err() {
            bus.disable_db("schema initialization failed in persist loop");
            continue;
        }

        let pending_requests = receiver.len().saturating_add(1);
        max_pending_requests = max_pending_requests.max(pending_requests);
        if pressure_cfg.enabled && pending_requests >= warn_threshold {
            if !queue_pressure {
                if pressure_cfg.should_emit_warning(last_warning_time) {
                    warn!(
                        "persist queue pressure high: pending={} / warn_threshold={} / capacity={} (max_pending={})",
                        pending_requests,
                        warn_threshold,
                        DB_PERSIST_QUEUE_CAPACITY,
                        max_pending_requests,
                    );
                    last_warning_time = Some(StdInstant::now());
                }
                queue_pressure = true;
            }
        } else if queue_pressure && pending_requests <= clear_threshold {
            queue_pressure = false;
        }

        let mut batch = Vec::with_capacity(DB_PERSIST_BATCH_SIZE);
        batch.push(first_request);

        let wait = sleep(TokioDuration::from_millis(DB_PERSIST_BATCH_WAIT_MS));
        tokio::pin!(wait);

        while batch.len() < DB_PERSIST_BATCH_SIZE && !channel_closed {
            tokio::select! {
                request = receiver.recv() => {
                    if let Some(request) = request {
                        batch.push(request);
                    } else {
                        channel_closed = true;
                    }
                }
                _ = &mut wait => {
                    break;
                }
            }
        }

        if persist_event_batch(&bus, &pool, batch).await.is_err() {
            break;
        }

        if channel_closed {
            break;
        }
    }
}

async fn persist_event_batch(
    bus: &EventBus,
    pool: &PgPool,
    batch: Vec<PersistRequest>,
) -> Result<(), ()> {
    if batch.is_empty() {
        return Ok(());
    }

    let mut grouped: HashMap<(String, usize, EventOverflowPolicy), Vec<PersistRequest>> =
        HashMap::new();

    for request in batch {
        let key = (
            request.event.username.clone(),
            request.max_records,
            request.overflow_policy,
        );
        grouped.entry(key).or_default().push(request);
    }

    for ((username, max_records, overflow_policy), mut requests) in grouped {
        if bus.active_pool().is_none() {
            return Ok(());
        }
        match overflow_policy {
            EventOverflowPolicy::DropNew => {
                if persist_drop_new_batch(bus, pool, &mut requests)
                    .await
                    .is_err()
                {
                    return Err(());
                }
            }
            EventOverflowPolicy::DropOldest => {
                if persist_oldest_batch(bus, pool, &username, max_records, &mut requests)
                    .await
                    .is_err()
                {
                    return Err(());
                }
            }
        }
    }

    Ok(())
}

async fn persist_drop_new_batch(
    bus: &EventBus,
    pool: &PgPool,
    requests: &mut Vec<PersistRequest>,
) -> Result<(), ()> {
    if requests.is_empty() {
        return Ok(());
    }
    requests.sort_by(|left, right| {
        left.event
            .timestamp
            .cmp(&right.event.timestamp)
            .then_with(|| left.event.event_type.cmp(&right.event.event_type))
    });

    let first_request = requests
        .first()
        .cloned()
        .expect("non-empty after empty check");
    let username = first_request.event.username.clone();
    let max_records = first_request.max_records;

    let current_count = match bus.cached_drop_new_count(&username).await {
        Some(cached_count) => cached_count,
        None => {
            let row =
                sqlx::query("SELECT COUNT(*) AS count FROM event_records WHERE username = $1")
                    .bind(&username)
                    .fetch_one(pool)
                    .await;

            match row {
                Ok(row) => {
                    let value: i64 = row.get("count");
                    let count = value.max(0) as usize;
                    bus.update_drop_new_count(&username, count).await;
                    count
                }
                Err(error) => {
                    bus.disable_db(&format!("count events for drop-new batch failed: {error}"));
                    return Err(());
                }
            }
        }
    };

    let remaining = max_records.saturating_sub(current_count);
    let inserted_count = requests.len().min(remaining);
    if inserted_count == 0 {
        return Ok(());
    }

    let mut query = QueryBuilder::<Postgres>::new(
        "INSERT INTO event_records (
            username, timestamp, source, event_type, category, severity, color, payload, is_read
        )",
    );
    query.push(" VALUES ");
    query.push_values(requests.iter().take(inserted_count), |mut b, request| {
        let event = &request.event;
        b.push_bind(&event.username)
            .push_bind(event.timestamp)
            .push_bind(&event.source)
            .push_bind(&event.event_type)
            .push_bind(to_category_str(event.category))
            .push_bind(to_severity_str(event.severity))
            .push_bind(to_color_str(event.color))
            .push_bind(event.payload.to_string())
            .push_bind(false);
    });

    if let Err(error) = query.build().execute(pool).await {
        bus.disable_db(&format!("batch insert failed for user {username}: {error}"));
        return Err(());
    }

    bus.update_drop_new_count(
        &username,
        current_count
            .saturating_add(inserted_count)
            .min(max_records),
    )
    .await;

    Ok(())
}

async fn persist_oldest_batch(
    bus: &EventBus,
    pool: &PgPool,
    username: &str,
    max_records: usize,
    requests: &mut Vec<PersistRequest>,
) -> Result<(), ()> {
    requests.sort_by(|left, right| {
        left.event
            .timestamp
            .cmp(&right.event.timestamp)
            .then_with(|| left.event.event_type.cmp(&right.event.event_type))
    });

    let mut query = QueryBuilder::<Postgres>::new(
        "INSERT INTO event_records (
            username, timestamp, source, event_type, category, severity, color, payload, is_read
        )",
    );
    query.push(" VALUES ");
    query.push_values(requests.iter(), |mut b, request| {
        let event = &request.event;
        b.push_bind(&event.username)
            .push_bind(event.timestamp)
            .push_bind(&event.source)
            .push_bind(&event.event_type)
            .push_bind(to_category_str(event.category))
            .push_bind(to_severity_str(event.severity))
            .push_bind(to_color_str(event.color))
            .push_bind(event.payload.to_string())
            .push_bind(false);
    });

    if let Err(error) = query.build().execute(pool).await {
        bus.disable_db(&format!("batch insert failed for user {username}: {error}"));
        return Err(());
    }

    if let Err(error) = sqlx::query(
        "DELETE FROM event_records
         WHERE id IN (
             SELECT id
             FROM event_records
             WHERE username = $1
             ORDER BY timestamp DESC, id DESC
             OFFSET $2
         )",
    )
    .bind(username)
    .bind(max_records as i64)
    .execute(pool)
    .await
    {
        bus.disable_db(&format!(
            "trim events in batch failed for user {username}: {error}"
        ));
        return Err(());
    }

    Ok(())
}

pub(crate) async fn persist_event_to_db(
    bus: &EventBus,
    pool: &PgPool,
    event: Event,
    max_records: usize,
    overflow_policy: EventOverflowPolicy,
) {
    let request = PersistRequest {
        event,
        max_records,
        overflow_policy,
    };

    if persist_event_batch(bus, pool, vec![request]).await.is_err() {
        bus.disable_db("persist event in fallback batch failed");
    }
}

pub(crate) async fn snapshot_from_db_with_filters(
    pool: &PgPool,
    username: &str,
    category: Option<&str>,
    severity: Option<&str>,
    limit: Option<usize>,
    since_minutes: Option<i64>,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
) -> Result<Vec<Event>, sqlx::Error> {
    let mut query = String::from(
        "SELECT username, timestamp, source, event_type, category, severity, color, payload FROM event_records WHERE username = $1",
    );

    let category_filter = category
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| raw.to_ascii_lowercase())
        .filter(|raw| matches!(raw.as_str(), "kernel" | "platform"));

    let severity_filter = severity
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| raw.to_ascii_lowercase())
        .filter(|raw| matches!(raw.as_str(), "success" | "warning" | "error"));

    let since_cutoff = since_minutes
        .filter(|minutes| *minutes > 0)
        .map(|minutes| Utc::now() - Duration::minutes(minutes));

    let checked_limit = limit
        .filter(|value| *value > 0)
        .map(|value| value.min(DB_PERSIST_QUERY_LIMIT))
        .map(|value| value as i64);

    let mut next_param = 2usize;

    if category_filter.is_some() {
        query.push_str(&format!(" AND category = ${next_param}"));
        next_param += 1;
    }
    if severity_filter.is_some() {
        query.push_str(&format!(" AND severity = ${next_param}"));
        next_param += 1;
    }
    if since_cutoff.is_some() {
        query.push_str(&format!(" AND timestamp >= ${next_param}"));
        next_param += 1;
    }
    if start.is_some() {
        query.push_str(&format!(" AND timestamp >= ${next_param}"));
        next_param += 1;
    }
    if end.is_some() {
        query.push_str(&format!(" AND timestamp <= ${next_param}"));
        next_param += 1;
    }

    query.push_str(" ORDER BY timestamp DESC");
    if checked_limit.is_some() {
        query.push_str(&format!(" LIMIT ${next_param}"));
    }

    let mut db_query = sqlx::query(&query).bind(username);
    if let Some(value) = category_filter {
        db_query = db_query.bind(value);
    }
    if let Some(value) = severity_filter {
        db_query = db_query.bind(value);
    }
    if let Some(value) = since_cutoff {
        db_query = db_query.bind(value);
    }
    if let Some(value) = start {
        db_query = db_query.bind(value);
    }
    if let Some(value) = end {
        db_query = db_query.bind(value);
    }
    if let Some(value) = checked_limit {
        db_query = db_query.bind(value);
    }

    let rows = db_query.fetch_all(pool).await?;
    let mut output = Vec::with_capacity(rows.len());
    for row in rows {
        let payload = row
            .try_get::<Json<serde_json::Value>, _>("payload")
            .map(|value| value.0)
            .unwrap_or_else(|_| serde_json::json!({}));

        output.push(Event {
            username: row.get("username"),
            timestamp: row.get("timestamp"),
            source: row.get("source"),
            event_type: row.get("event_type"),
            category: parse_category(row.get::<String, _>("category").as_str()),
            severity: parse_severity(row.get::<String, _>("severity").as_str()),
            color: parse_color(row.get::<String, _>("color").as_str()),
            payload,
        });
    }

    output.reverse();
    Ok(output)
}

pub(crate) async fn delete_events_from_db_with_filters(
    pool: &PgPool,
    username: &str,
    category: Option<&str>,
    severity: Option<&str>,
    since_minutes: Option<i64>,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
) -> Result<u64, sqlx::Error> {
    let mut query = String::from("DELETE FROM event_records WHERE username = $1");

    let category_filter = category
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| raw.to_ascii_lowercase())
        .filter(|raw| matches!(raw.as_str(), "kernel" | "platform"));

    let severity_filter = severity
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| raw.to_ascii_lowercase())
        .filter(|raw| matches!(raw.as_str(), "success" | "warning" | "error"));

    let since_cutoff = since_minutes
        .filter(|minutes| *minutes > 0)
        .map(|minutes| Utc::now() - Duration::minutes(minutes));

    let mut next_param = 2usize;

    if category_filter.is_some() {
        query.push_str(&format!(" AND category = ${next_param}"));
        next_param += 1;
    }
    if severity_filter.is_some() {
        query.push_str(&format!(" AND severity = ${next_param}"));
        next_param += 1;
    }
    if since_cutoff.is_some() {
        query.push_str(&format!(" AND timestamp >= ${next_param}"));
        next_param += 1;
    }
    if start.is_some() {
        query.push_str(&format!(" AND timestamp >= ${next_param}"));
        next_param += 1;
    }
    if end.is_some() {
        query.push_str(&format!(" AND timestamp <= ${next_param}"));
    }

    let mut db_query = sqlx::query(&query).bind(username);
    if let Some(value) = category_filter {
        db_query = db_query.bind(value);
    }
    if let Some(value) = severity_filter {
        db_query = db_query.bind(value);
    }
    if let Some(value) = since_cutoff {
        db_query = db_query.bind(value);
    }
    if let Some(value) = start {
        db_query = db_query.bind(value);
    }
    if let Some(value) = end {
        db_query = db_query.bind(value);
    }

    let result = db_query.execute(pool).await?;
    Ok(result.rows_affected())
}

pub(crate) async fn delete_all_events_for_user(
    pool: &PgPool,
    username: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM event_records WHERE username = $1")
        .bind(username)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

fn parse_category(raw: &str) -> crate::models::event::EventCategory {
    match raw {
        "kernel" => crate::models::event::EventCategory::Kernel,
        _ => crate::models::event::EventCategory::Platform,
    }
}

fn parse_severity(raw: &str) -> crate::models::event::EventSeverity {
    match raw {
        "success" => crate::models::event::EventSeverity::Success,
        "warning" => crate::models::event::EventSeverity::Warning,
        _ => crate::models::event::EventSeverity::Error,
    }
}

fn parse_color(raw: &str) -> crate::models::event::EventColor {
    match raw {
        "green" => crate::models::event::EventColor::Green,
        "yellow" => crate::models::event::EventColor::Yellow,
        _ => crate::models::event::EventColor::Red,
    }
}
