use crate::services::event_bus_codec::{to_category_str, to_color_str, to_severity_str};
use crate::services::event_bus_db;
use crate::services::event_bus_filter::filter_events;
use crate::services::event_bus_policy::{parse_policy, policy_to_str};
use crate::sqlx_compat::{PgPool, PgPoolOptions, Postgres, QueryBuilder, Row};
use chrono::{DateTime, Utc};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration as StdDuration, Instant},
};

use crate::sqlx_compat as sqlx;
use tokio::sync::{broadcast, mpsc, OnceCell, RwLock};

use crate::models::event::Event;

mod event_bus_schema;

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

#[derive(Debug, Clone, Copy, Default)]
pub struct EventQueryFilters<'a> {
    pub category: Option<&'a str>,
    pub severity: Option<&'a str>,
    pub limit: Option<usize>,
    pub since_minutes: Option<i64>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

impl<'a> EventQueryFilters<'a> {
    pub(crate) fn to_db_query(self, username: &'a str) -> event_bus_db::EventQueryFilter<'a> {
        event_bus_db::EventQueryFilter {
            username,
            category: self.category,
            severity: self.severity,
            limit: self.limit,
            since_minutes: self.since_minutes,
            start: self.start,
            end: self.end,
        }
    }
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
        self.snapshot_for_user_filtered(username, EventQueryFilters::default())
            .await
    }

    pub(crate) async fn snapshot_for_user_filtered(
        &self,
        username: &str,
        filters: EventQueryFilters<'_>,
    ) -> Vec<Event> {
        let EventQueryFilters {
            category,
            severity,
            limit,
            since_minutes,
            start,
            end,
        } = filters;
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                let filters = EventQueryFilters {
                    category,
                    severity,
                    limit,
                    since_minutes,
                    start,
                    end,
                };
                match event_bus_db::snapshot_from_db_with_filters(
                    pool,
                    filters.to_db_query(username),
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
                let filters = EventQueryFilters {
                    category,
                    severity,
                    limit: None,
                    since_minutes,
                    start,
                    end,
                };
                match event_bus_db::delete_events_from_db_with_filters(
                    pool,
                    filters.to_db_query(username),
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

                if !truncated.is_empty() {
                    let _ = event_bus_db::execute_sql_with_retry(
                        self,
                        &format!("replace events insert for user {username}"),
                        || async {
                            let mut query = QueryBuilder::<Postgres>::new(
                                "INSERT INTO event_records (
                                    username, timestamp, source, event_type, category, severity, color, payload, is_read
                                )",
                            );
                            query.push(" VALUES ");
                            query.push_values(truncated.iter(), |mut builder, event| {
                                builder
                                    .push_bind(&event.username)
                                    .push_bind(event.timestamp)
                                    .push_bind(&event.source)
                                    .push_bind(&event.event_type)
                                    .push_bind(to_category_str(event.category))
                                    .push_bind(to_severity_str(event.severity))
                                    .push_bind(to_color_str(event.color))
                                    .push_bind(event.payload.to_string())
                                    .push_bind(true);
                            });
                            query.build().execute(pool).await
                        },
                    )
                    .await;
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
}
