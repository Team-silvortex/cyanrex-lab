use crate::services::event_bus_db;
use crate::services::event_bus_filter::latest_matching_events;
use crate::services::event_bus_policy::parse_policy;
use crate::sqlx_compat::{PgPool, PgPoolOptions, Row};
use chrono::{DateTime, Utc};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration as StdDuration, Instant},
};

use crate::sqlx_compat as sqlx;
use tokio::sync::{broadcast, mpsc, OnceCell, RwLock};

use crate::models::event::Event;

mod delete_operation;
mod deletion;
mod event_bus_schema;
mod mutations;
mod persistence;
#[cfg(test)]
mod read_bench;
mod subscriptions;
#[cfg(test)]
mod tests;

pub(crate) use subscriptions::{UserEventSubscription, UserFanout};

pub(crate) const DB_PERSIST_QUEUE_CAPACITY: usize = 2_048;
const DB_PERSIST_DROP_NEW_COUNT_TTL: StdDuration = StdDuration::from_secs(1);

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    user_fanout: UserFanout,
    history: Arc<RwLock<HashMap<String, VecDeque<Event>>>>,
    unread: Arc<RwLock<HashMap<String, usize>>>,
    settings: Arc<RwLock<HashMap<String, UserEventSettings>>>,
    db_pool: Option<PgPool>,
    schema_ready: Arc<OnceCell<()>>,
    db_disabled: Arc<AtomicBool>,
    persist_sender: mpsc::Sender<event_bus_db::PersistMessage>,
    mutation_gates: Arc<std::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
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
            user_fanout: UserFanout::new(buffer),
            history: Arc::new(RwLock::new(HashMap::new())),
            unread: Arc::new(RwLock::new(HashMap::new())),
            settings: Arc::new(RwLock::new(HashMap::new())),
            db_pool,
            schema_ready: Arc::new(OnceCell::new()),
            db_disabled: Arc::new(AtomicBool::new(false)),
            persist_sender,
            mutation_gates: Arc::new(std::sync::Mutex::new(HashMap::new())),
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
        let _admission = self.mutation_admission(&event.username).await;
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
            // Publish history and its unread suffix together, in the same lock order as deletion.
            let mut unread = self.unread.write().await;
            // Evict before insertion: a full deque must not grow or shift retained events.
            while bucket.len() >= max_records {
                bucket.pop_front();
            }
            bucket.push_back(event.clone());
            let counter = unread.entry(event.username.clone()).or_insert(0);
            *counter = counter.saturating_add(1).min(max_records);
        }
        self.user_fanout.publish(&event);
        // Preserve the legacy global service API without cloning for an unused channel.
        if self.sender.receiver_count() > 0 {
            let _ = self.sender.send(event.clone());
        }
        if self.active_pool().is_some() {
            let request = event_bus_db::new_persist_request(
                event,
                max_records,
                user_settings.overflow_policy,
            );
            if self
                .persist_sender
                .try_send(event_bus_db::PersistMessage::Event(request))
                .is_err()
            {
                // Never bypass the bounded, ordered writer with an unbounded spawned task.
                self.disable_db(
                    "persist queue full or closed; using bounded volatile history until restart",
                );
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
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
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

        self.snapshot_from_history_filtered(username, filters).await
    }

    async fn snapshot_from_history_filtered(
        &self,
        username: &str,
        filters: EventQueryFilters<'_>,
    ) -> Vec<Event> {
        let max_records = self.settings_for_user(username).await.max_records.max(1);
        let history = self.history.read().await;
        history
            .get(username)
            .map(|events| {
                let mut selected: Vec<_> =
                    latest_matching_events(events.iter(), max_records, filters).collect();
                // Reverse lightweight references, then clone FIFO into an exactly sized result.
                selected.reverse();
                selected.into_iter().cloned().collect()
            })
            .unwrap_or_default()
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
                match self
                    .query_with_deadline(
                        sqlx::query(
                            "SELECT max_records, overflow_policy
                     FROM event_user_settings
                     WHERE username = $1",
                        )
                        .bind(username)
                        .fetch_optional(pool),
                    )
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
        // A cold SQL read must not overwrite settings published while it was awaiting I/O.
        *cache.entry(username.to_string()).or_insert(value)
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
