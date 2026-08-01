use crate::services::event_bus_codec::{to_category_str, to_color_str, to_severity_str};
use crate::services::event_bus_db;
use crate::services::event_bus_filter::filter_events;
use crate::services::event_bus_policy::{parse_policy, policy_to_str};
use chrono::{DateTime, Utc};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration as StdDuration, Instant},
};

use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tokio::sync::{broadcast, mpsc, OnceCell, RwLock};

use crate::models::event::Event;

pub(crate) const DB_PERSIST_QUEUE_CAPACITY: usize = 2_048;
const DB_PERSIST_DROP_NEW_COUNT_TTL: StdDuration = StdDuration::from_secs(1);

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    history: Arc<RwLock<HashMap<String, Vec<Event>>>>,
    unread: Arc<RwLock<HashMap<String, usize>>>,
    settings: Arc<RwLock<HashMap<String, UserEventSettings>>>,
    db_pool: Option<PgPool>,
    schema_ready: Arc<OnceCell<()>>,
    db_disabled: Arc<AtomicBool>,
    persist_sender: mpsc::Sender<event_bus_db::PersistRequest>,
    drop_new_count_cache: Arc<RwLock<HashMap<String, (usize, Instant)>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventOverflowPolicy {
    DropOldest,
    DropNew,
}

#[derive(Debug, Clone, Copy)]
pub struct UserEventSettings {
    pub max_records: usize,
    pub overflow_policy: EventOverflowPolicy,
}

impl Default for UserEventSettings {
    fn default() -> Self {
        Self {
            max_records: 500,
            overflow_policy: EventOverflowPolicy::DropOldest,
        }
    }
}

impl EventBus {
    pub fn new(buffer: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer);
        let db_pool = std::env::var("DATABASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .and_then(|url| {
                PgPoolOptions::new()
                    .max_connections(5)
                    .connect_lazy(&url)
                    .ok()
            });
        let (persist_sender, persist_receiver) = mpsc::channel(DB_PERSIST_QUEUE_CAPACITY);

        let bus = Self {
            sender,
            history: Arc::new(RwLock::new(HashMap::new())),
            unread: Arc::new(RwLock::new(HashMap::new())),
            settings: Arc::new(RwLock::new(HashMap::new())),
            db_pool,
            schema_ready: Arc::new(OnceCell::new()),
            db_disabled: Arc::new(AtomicBool::new(false)),
            persist_sender,
            drop_new_count_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        if let Some(pool) = bus.db_pool.clone() {
            let writer_bus = bus.clone();
            tokio::spawn(async move {
                event_bus_db::run_persist_loop(writer_bus, pool, persist_receiver).await;
            });
        }

        bus
    }

    pub async fn publish(&self, event: Event) {
        let user_settings = self.settings_for_user(&event.username).await;
        let max_records = user_settings.max_records.max(1);

        {
            let mut history = self.history.write().await;
            let bucket = history.entry(event.username.clone()).or_default();
            if user_settings.overflow_policy == EventOverflowPolicy::DropNew
                && bucket.len() >= max_records
            {
                return;
            }
            bucket.push(event.clone());
            if bucket.len() > max_records {
                bucket.drain(..bucket.len() - max_records);
            }
        }
        let _ = self.sender.send(event.clone());
        {
            let mut unread = self.unread.write().await;
            let counter = unread.entry(event.username.clone()).or_insert(0);
            *counter = counter.saturating_add(1);
            if *counter > max_records {
                *counter = max_records;
            }
        }

        if let Some(pool) = self.active_pool() {
            let queue_request = event_bus_db::new_persist_request(
                event.clone(),
                max_records,
                user_settings.overflow_policy,
            );

            if let Err(error) = self.persist_sender.try_send(queue_request) {
                let queue_request = match error {
                    tokio::sync::mpsc::error::TrySendError::Full(request)
                    | tokio::sync::mpsc::error::TrySendError::Closed(request) => request,
                };
                let bus = self.clone();
                let pool = pool.clone();
                tokio::spawn(async move {
                    event_bus_db::persist_event_to_db(
                        &bus,
                        &pool,
                        queue_request.event,
                        queue_request.max_records,
                        queue_request.overflow_policy,
                    )
                    .await;
                });
            }
        }
    }

    pub(crate) async fn cached_drop_new_count(&self, username: &str) -> Option<usize> {
        let now = Instant::now();
        self.drop_new_count_cache
            .read()
            .await
            .get(username)
            .and_then(|(count, updated_at)| {
                (now.duration_since(*updated_at) <= DB_PERSIST_DROP_NEW_COUNT_TTL).then_some(*count)
            })
    }

    pub(crate) async fn update_drop_new_count(&self, username: &str, count: usize) {
        let mut cache = self.drop_new_count_cache.write().await;
        cache.insert(username.to_string(), (count, Instant::now()));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    pub async fn snapshot_for_user(&self, username: &str) -> Vec<Event> {
        self.snapshot_for_user_filtered(username, None, None, None, None, None, None)
            .await
    }

    pub async fn snapshot_for_user_filtered(
        &self,
        username: &str,
        category: Option<&str>,
        severity: Option<&str>,
        limit: Option<usize>,
        since_minutes: Option<i64>,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Vec<Event> {
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                match event_bus_db::snapshot_from_db_with_filters(
                    pool,
                    username,
                    category,
                    severity,
                    limit,
                    since_minutes,
                    start,
                    end,
                )
                .await
                {
                    Ok(events) => return events,
                    Err(error) => self.disable_db(&format!("snapshot query failed: {error}")),
                }
            }
        }

        let history = self.snapshot_from_history(username).await;
        filter_events(
            history,
            category,
            severity,
            limit,
            since_minutes,
            start,
            end,
        )
    }

    pub async fn delete_user_events_filtered(
        &self,
        username: &str,
        category: Option<&str>,
        severity: Option<&str>,
        since_minutes: Option<i64>,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> usize {
        let has_filter = category.is_some()
            || severity.is_some()
            || since_minutes.is_some_and(|minutes| minutes > 0)
            || start.is_some()
            || end.is_some();

        if !has_filter {
            if let Some(pool) = self.active_pool() {
                if self.ensure_schema().await.is_ok() {
                    match event_bus_db::delete_all_events_for_user(pool, username).await {
                        Ok(deleted) => {
                            {
                                let mut history = self.history.write().await;
                                history.remove(username);
                            }
                            let mut unread = self.unread.write().await;
                            unread.insert(username.to_string(), 0);
                            return deleted as usize;
                        }
                        Err(error) => {
                            self.disable_db(&format!("delete all events failed: {error}"))
                        }
                    }
                }
            }

            let deleted = {
                let mut history = self.history.write().await;
                history.remove(username).map_or(0, |events| events.len())
            };
            if deleted > 0 {
                let mut unread = self.unread.write().await;
                unread.insert(username.to_string(), 0);
            }
            return deleted;
        }

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                match event_bus_db::delete_events_from_db_with_filters(
                    pool,
                    username,
                    category,
                    severity,
                    since_minutes,
                    start,
                    end,
                )
                .await
                {
                    Ok(deleted) => {
                        if deleted > 0 {
                            self.delete_from_history_filtered(
                                username,
                                category,
                                severity,
                                since_minutes,
                                start,
                                end,
                            )
                            .await;
                        }
                        return deleted as usize;
                    }
                    Err(error) => {
                        self.disable_db(&format!("delete filtered events failed: {error}"))
                    }
                }
            }
        }

        self.delete_from_history_filtered(username, category, severity, since_minutes, start, end)
            .await
    }

    async fn snapshot_from_history(&self, username: &str) -> Vec<Event> {
        let max_records = self.settings_for_user(username).await.max_records.max(1) as i64;
        let history = self.history.read().await;
        let mut data = history.get(username).cloned().unwrap_or_default();
        let max = max_records as usize;
        if data.len() > max {
            data = data.split_off(data.len() - max);
        }
        data
    }

    async fn delete_from_history_filtered(
        &self,
        username: &str,
        category: Option<&str>,
        severity: Option<&str>,
        since_minutes: Option<i64>,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> usize {
        let events = self.snapshot_from_history(username).await;
        if events.is_empty() {
            return 0;
        }

        let original_len = events.len();
        let events = filter_events(events, category, severity, None, since_minutes, start, end);

        let deleted = original_len.saturating_sub(events.len());
        if deleted > 0 {
            self.replace_user_events(username, events).await;
        }
        deleted
    }

    pub async fn unread_count_for_user(&self, username: &str) -> usize {
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                match sqlx::query("SELECT COUNT(*) AS count FROM event_records WHERE username = $1 AND is_read = false")
                    .bind(username)
                    .fetch_one(pool)
                    .await
                {
                    Ok(row) => {
                        let count: i64 = row.get("count");
                        return count.max(0) as usize;
                    }
                    Err(error) => self.disable_db(&format!("unread count query failed: {error}")),
                }
            }
        }

        let unread = self.unread.read().await;
        unread.get(username).copied().unwrap_or(0)
    }

    pub async fn mark_all_read_for_user(&self, username: &str) {
        {
            let mut unread = self.unread.write().await;
            unread.insert(username.to_string(), 0);
        }

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                if let Err(error) = sqlx::query(
                    "UPDATE event_records SET is_read = true WHERE username = $1 AND is_read = false",
                )
                .bind(username)
                .execute(pool)
                .await
                {
                    self.disable_db(&format!("mark read failed: {error}"));
                }
            }
        }
    }

    pub async fn replace_user_events(&self, username: &str, events: Vec<Event>) {
        let max_records = self.settings_for_user(username).await.max_records.max(1);
        let mut truncated = events;
        if truncated.len() > max_records {
            truncated = truncated.split_off(truncated.len() - max_records);
        }
        {
            let mut history = self.history.write().await;
            history.insert(username.to_string(), truncated.clone());
        }
        {
            let mut unread = self.unread.write().await;
            unread.insert(username.to_string(), 0);
        }

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                if let Err(error) = sqlx::query("DELETE FROM event_records WHERE username = $1")
                    .bind(username)
                    .execute(pool)
                    .await
                {
                    self.disable_db(&format!("replace events delete failed: {error}"));
                    return;
                }

                for event in truncated {
                    if let Err(error) = sqlx::query(
                        "INSERT INTO event_records (username, timestamp, source, event_type, category, severity, color, payload, is_read)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb, true)",
                    )
                    .bind(&event.username)
                    .bind(event.timestamp)
                    .bind(&event.source)
                    .bind(&event.event_type)
                    .bind(to_category_str(event.category))
                    .bind(to_severity_str(event.severity))
                    .bind(to_color_str(event.color))
                    .bind(event.payload.to_string())
                    .execute(pool)
                    .await
                    {
                        self.disable_db(&format!("replace events insert failed: {error}"));
                        return;
                    }
                }
            }
        }
    }

    pub async fn settings_for_user(&self, username: &str) -> UserEventSettings {
        {
            let cache = self.settings.read().await;
            if let Some(value) = cache.get(username) {
                return *value;
            }
        }

        let mut value = UserEventSettings::default();
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                match sqlx::query(
                    "SELECT max_records, overflow_policy
                     FROM event_user_settings
                     WHERE username = $1",
                )
                .bind(username)
                .fetch_optional(pool)
                .await
                {
                    Ok(Some(row)) => {
                        let max_records: i64 = row.get("max_records");
                        let overflow_policy: String = row.get("overflow_policy");
                        value.max_records = (max_records.max(1) as usize).clamp(50, 50000);
                        value.overflow_policy = parse_policy(&overflow_policy);
                    }
                    Ok(None) => {}
                    Err(error) => self.disable_db(&format!("load event settings failed: {error}")),
                }
            }
        }

        let mut cache = self.settings.write().await;
        cache.insert(username.to_string(), value);
        value
    }

    pub async fn update_settings_for_user(
        &self,
        username: &str,
        max_records: usize,
        overflow_policy: EventOverflowPolicy,
    ) -> Result<UserEventSettings, String> {
        let settings = UserEventSettings {
            max_records: max_records.clamp(50, 50000),
            overflow_policy,
        };

        {
            let mut cache = self.settings.write().await;
            cache.insert(username.to_string(), settings);
        }

        {
            let mut history = self.history.write().await;
            if let Some(bucket) = history.get_mut(username) {
                if bucket.len() > settings.max_records {
                    bucket.drain(..bucket.len() - settings.max_records);
                }
            }
        }
        {
            let mut unread = self.unread.write().await;
            if let Some(counter) = unread.get_mut(username) {
                if *counter > settings.max_records {
                    *counter = settings.max_records;
                }
            }
        }

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                sqlx::query(
                    "INSERT INTO event_user_settings (username, max_records, overflow_policy)
                     VALUES ($1, $2, $3)
                     ON CONFLICT (username)
                     DO UPDATE SET max_records = EXCLUDED.max_records, overflow_policy = EXCLUDED.overflow_policy",
                )
                .bind(username)
                .bind(settings.max_records as i64)
                .bind(policy_to_str(settings.overflow_policy))
                .execute(pool)
                .await
                .map_err(|error| format!("update event settings failed: {error}"))?;

                sqlx::query(
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
                .bind(settings.max_records as i64)
                .execute(pool)
                .await
                .map_err(|error| {
                    format!("trim event records after settings update failed: {error}")
                })?;
            }
        }

        Ok(settings)
    }

    pub(crate) fn active_pool(&self) -> Option<&PgPool> {
        if !crate::config::db_fallback_enabled() {
            return None;
        }
        if self.db_disabled.load(Ordering::Relaxed) {
            None
        } else {
            self.db_pool.as_ref()
        }
    }

    pub(crate) fn disable_db(&self, reason: &str) {
        tracing::warn!("disabling event db persistence: {reason}");
        self.db_disabled.store(true, Ordering::Relaxed);
    }

    pub(crate) async fn ensure_schema(&self) -> Result<(), sqlx::Error> {
        let Some(pool) = self.active_pool() else {
            return Ok(());
        };

        self.schema_ready
            .get_or_try_init(|| async {
                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS event_records (
                        id BIGSERIAL PRIMARY KEY,
                        username TEXT NOT NULL,
                        timestamp TIMESTAMPTZ NOT NULL,
                        source TEXT NOT NULL,
                        event_type TEXT NOT NULL,
                        category TEXT NOT NULL,
                        severity TEXT NOT NULL,
                        color TEXT NOT NULL,
                        payload JSONB NOT NULL,
                        is_read BOOLEAN NOT NULL DEFAULT false
                    )",
                )
                .execute(pool)
                .await?;

                sqlx::query("CREATE INDEX IF NOT EXISTS idx_event_records_user_time ON event_records(username, timestamp)")
                    .execute(pool)
                    .await?;
                sqlx::query("CREATE INDEX IF NOT EXISTS idx_event_records_user_unread ON event_records(username, is_read)")
                    .execute(pool)
                    .await?;
                sqlx::query("CREATE INDEX IF NOT EXISTS idx_event_records_user_category_time ON event_records(username, category, timestamp)")
                    .execute(pool)
                    .await?;
                sqlx::query("CREATE INDEX IF NOT EXISTS idx_event_records_user_severity_time ON event_records(username, severity, timestamp)")
                    .execute(pool)
                    .await?;
                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS event_user_settings (
                        username TEXT PRIMARY KEY,
                        max_records BIGINT NOT NULL,
                        overflow_policy TEXT NOT NULL
                    )",
                )
                .execute(pool)
                .await?;
                Ok(())
            })
            .await
            .map(|_| ())
    }
}
