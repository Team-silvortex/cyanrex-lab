use super::*;
use crate::services::event_bus_filter::{normalize_category_filter, normalize_severity_filter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventReadError {
    InvalidFilters,
    Unavailable,
}

impl EventQueryFilters<'_> {
    pub(crate) fn checked_for_read(mut self) -> Result<Self, EventReadError> {
        if (self.category.is_some() && normalize_category_filter(self.category).is_none())
            || (self.severity.is_some() && normalize_severity_filter(self.severity).is_none())
            || self.since_minutes.is_some_and(|value| value < 0)
        {
            return Err(EventReadError::InvalidFilters);
        }
        if let Some(minutes) = self.since_minutes.filter(|value| *value > 0) {
            let cutoff = chrono::Duration::try_minutes(minutes)
                .and_then(|duration| Utc::now().checked_sub_signed(duration))
                .ok_or(EventReadError::InvalidFilters)?;
            self.start = Some(self.start.map_or(cutoff, |start| start.max(cutoff)));
        }
        // Both backends receive one checked, frozen cutoff; zero still means no time window.
        self.since_minutes = None;
        if self
            .start
            .zip(self.end)
            .is_some_and(|(start, end)| start > end)
        {
            return Err(EventReadError::InvalidFilters);
        }
        Ok(self)
    }
}

impl EventBus {
    pub(super) fn authoritative_pool(&self) -> Result<Option<&PgPool>, EventReadError> {
        if !crate::config::db_fallback_enabled() || (!self.db_configured && self.db_pool.is_none())
        {
            return Ok(None);
        }
        // A configured but unavailable/latched store is not an intentionally memory-only lab.
        if self.db_disabled.load(Ordering::Relaxed) {
            return Err(EventReadError::Unavailable);
        }
        self.db_pool
            .as_ref()
            .map(Some)
            .ok_or(EventReadError::Unavailable)
    }

    pub(crate) async fn read_snapshot_for_user_filtered(
        &self,
        username: &str,
        filters: EventQueryFilters<'_>,
    ) -> Result<Vec<Event>, EventReadError> {
        let filters = filters.checked_for_read()?;
        let pool = self.authoritative_pool()?;
        self.read_with_deadline(pool.is_some(), async {
            if let Some(pool) = pool {
                self.ensure_schema_on(pool).await?;
                event_bus_db::snapshot_from_db_with_filters(pool, filters.to_db_query(username))
                    .await
            } else {
                Ok(self.snapshot_from_history_filtered(username, filters).await)
            }
        })
        .await
    }

    pub(crate) async fn read_unread_count_for_user(
        &self,
        username: &str,
    ) -> Result<usize, EventReadError> {
        let pool = self.authoritative_pool()?;
        self.read_with_deadline(pool.is_some(), async {
            if let Some(pool) = pool {
                self.ensure_schema_on(pool).await?;
                let row = sqlx::query("SELECT COUNT(*) AS count FROM event_records WHERE username = $1 AND is_read = false")
                    .bind(username).fetch_one(pool).await?;
                let count: i64 = row.try_get("count")?;
                Ok(count.max(0) as usize)
            } else {
                Ok(self.unread.read().await.get(username).copied().unwrap_or(0))
            }
        }).await
    }

    pub(super) async fn read_with_deadline<T>(
        &self,
        selected_sql: bool,
        query: impl std::future::Future<Output = Result<T, sqlx::Error>>,
    ) -> Result<T, EventReadError> {
        match tokio::time::timeout(StdDuration::from_secs(10), query).await {
            Ok(Ok(value)) => {
                // An independent writer may have latched fallback while this read was waiting.
                if selected_sql && self.db_disabled.load(Ordering::Relaxed) {
                    Err(EventReadError::Unavailable)
                } else {
                    Ok(value)
                }
            }
            Ok(Err(error)) => {
                tracing::warn!(%error, "event storage read failed; persistence remains retryable");
                Err(EventReadError::Unavailable)
            }
            Err(_) => {
                tracing::warn!(
                    "event storage read deadline exceeded; persistence remains retryable"
                );
                Err(EventReadError::Unavailable)
            }
        }
    }
}
