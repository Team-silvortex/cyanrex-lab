use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use sqlx::{postgres::PgPoolOptions, types::Json, PgPool, Row};
use tokio::sync::{broadcast, OnceCell, RwLock};

use crate::models::event::{Event, EventCategory, EventColor, EventSeverity};

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    history: Arc<RwLock<HashMap<String, Vec<Event>>>>,
    unread: Arc<RwLock<HashMap<String, usize>>>,
    settings: Arc<RwLock<HashMap<String, UserEventSettings>>>,
    db_pool: Option<PgPool>,
    schema_ready: Arc<OnceCell<()>>,
    db_disabled: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

        Self {
            sender,
            history: Arc::new(RwLock::new(HashMap::new())),
            unread: Arc::new(RwLock::new(HashMap::new())),
            settings: Arc::new(RwLock::new(HashMap::new())),
            db_pool,
            schema_ready: Arc::new(OnceCell::new()),
            db_disabled: Arc::new(AtomicBool::new(false)),
        }
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
            while bucket.len() > max_records {
                bucket.remove(0);
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
            if self.ensure_schema().await.is_ok() {
                if user_settings.overflow_policy == EventOverflowPolicy::DropNew {
                    match sqlx::query("SELECT COUNT(*) AS count FROM event_records WHERE username = $1")
                        .bind(&event.username)
                        .fetch_one(pool)
                        .await
                    {
                        Ok(row) => {
                            let count: i64 = row.get("count");
                            if count >= max_records as i64 {
                                return;
                            }
                        }
                        Err(error) => {
                            self.disable_db(&format!("event count query failed: {error}"));
                            return;
                        }
                    }
                }

                if let Err(error) = sqlx::query(
                    "INSERT INTO event_records (username, timestamp, source, event_type, category, severity, color, payload, is_read)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb, false)",
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
                    self.disable_db(&format!("insert event failed: {error}"));
                    return;
                }

                if user_settings.overflow_policy == EventOverflowPolicy::DropOldest {
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
                    .bind(&event.username)
                    .bind(max_records as i64)
                    .execute(pool)
                    .await
                    {
                        self.disable_db(&format!("trim events failed: {error}"));
                    }
                }
            }
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    pub async fn snapshot_for_user(&self, username: &str) -> Vec<Event> {
        let max_records = self.settings_for_user(username).await.max_records.max(1) as i64;
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                match sqlx::query(
                    "SELECT username, timestamp, source, event_type, category, severity, color, payload
                     FROM event_records
                     WHERE username = $1
                     ORDER BY timestamp ASC
                     LIMIT $2",
                )
                .bind(username)
                .bind(max_records)
                .fetch_all(pool)
                .await
                {
                    Ok(rows) => {
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
                        return output;
                    }
                    Err(error) => self.disable_db(&format!("snapshot query failed: {error}")),
                }
            }
        }

        let history = self.history.read().await;
        let mut data = history.get(username).cloned().unwrap_or_default();
        let max = max_records as usize;
        if data.len() > max {
            data = data.split_off(data.len() - max);
        }
        data
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
                while bucket.len() > settings.max_records {
                    bucket.remove(0);
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
                .map_err(|error| format!("trim event records after settings update failed: {error}"))?;
            }
        }

        Ok(settings)
    }

    fn active_pool(&self) -> Option<&PgPool> {
        if self.db_disabled.load(Ordering::Relaxed) {
            None
        } else {
            self.db_pool.as_ref()
        }
    }

    fn disable_db(&self, reason: &str) {
        tracing::warn!("disabling event db persistence: {reason}");
        self.db_disabled.store(true, Ordering::Relaxed);
    }

    async fn ensure_schema(&self) -> Result<(), sqlx::Error> {
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

fn policy_to_str(policy: EventOverflowPolicy) -> &'static str {
    match policy {
        EventOverflowPolicy::DropOldest => "drop_oldest",
        EventOverflowPolicy::DropNew => "drop_new",
    }
}

fn parse_policy(value: &str) -> EventOverflowPolicy {
    if value.eq_ignore_ascii_case("drop_new") {
        EventOverflowPolicy::DropNew
    } else {
        EventOverflowPolicy::DropOldest
    }
}

fn to_category_str(category: EventCategory) -> &'static str {
    match category {
        EventCategory::Kernel => "kernel",
        EventCategory::Platform => "platform",
    }
}

fn to_severity_str(severity: EventSeverity) -> &'static str {
    match severity {
        EventSeverity::Success => "success",
        EventSeverity::Warning => "warning",
        EventSeverity::Error => "error",
    }
}

fn to_color_str(color: EventColor) -> &'static str {
    match color {
        EventColor::Green => "green",
        EventColor::Yellow => "yellow",
        EventColor::Red => "red",
    }
}

fn parse_category(raw: &str) -> EventCategory {
    match raw {
        "kernel" => EventCategory::Kernel,
        _ => EventCategory::Platform,
    }
}

fn parse_severity(raw: &str) -> EventSeverity {
    match raw {
        "success" => EventSeverity::Success,
        "warning" => EventSeverity::Warning,
        _ => EventSeverity::Error,
    }
}

fn parse_color(raw: &str) -> EventColor {
    match raw {
        "green" => EventColor::Green,
        "yellow" => EventColor::Yellow,
        _ => EventColor::Red,
    }
}
