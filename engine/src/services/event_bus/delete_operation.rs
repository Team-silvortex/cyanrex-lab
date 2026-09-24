use super::*;

impl EventBus {
    /// Legacy best-effort helper; HTTP uses delete_user_events_confirmed instead.
    pub async fn delete_user_events_filtered(
        &self,
        username: &str,
        category: Option<&str>,
        severity: Option<&str>,
        since_minutes: Option<i64>,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> usize {
        let username = username.to_owned();
        let category = category.map(str::to_owned);
        let severity = severity.map(str::to_owned);
        self.with_persistence_barrier(username.clone(), move |bus| async move {
            let deleted = bus
                .delete_user_events_inner(
                    &username,
                    category.as_deref(),
                    severity.as_deref(),
                    since_minutes,
                    start,
                    end,
                )
                .await;
            bus.drop_new_count_cache.write().await.remove(&username);
            bus.drop_new_admitted.write().await.remove(&username);
            deleted
        })
        .await
    }

    async fn delete_user_events_inner(
        &self,
        username: &str,
        category: Option<&str>,
        severity: Option<&str>,
        since_minutes: Option<i64>,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> usize {
        use crate::services::event_bus_filter::{
            normalize_category_filter, normalize_severity_filter,
        };
        if (category.is_some() && normalize_category_filter(category).is_none())
            || (severity.is_some() && normalize_severity_filter(severity).is_none())
            || since_minutes.is_some_and(|minutes| minutes < 0)
        {
            return 0;
        }
        let mut start = start;
        if let Some(minutes) = since_minutes.filter(|minutes| *minutes > 0) {
            let Some(cutoff) = chrono::Duration::try_minutes(minutes)
                .and_then(|duration| Utc::now().checked_sub_signed(duration))
            else {
                return 0;
            };
            start = Some(start.map_or(cutoff, |value| value.max(cutoff)));
        }
        if start.zip(end).is_some_and(|(start, end)| start > end) {
            return 0;
        }
        // Both backends must use the same frozen cutoff, including near a minute boundary.
        let since_minutes = None;
        let has_filter = category.is_some()
            || severity.is_some()
            || since_minutes.is_some_and(|minutes| minutes > 0)
            || start.is_some()
            || end.is_some();

        if !has_filter {
            if let Some(pool) = self.active_pool() {
                if self.ensure_schema().await.is_ok() {
                    match self
                        .query_with_deadline(event_bus_db::delete_all_events_for_user(
                            pool, username,
                        ))
                        .await
                    {
                        Ok(deleted) => {
                            self.delete_from_history_filtered(
                                username, None, None, None, None, None,
                            )
                            .await;
                            return deleted as usize;
                        }
                        Err(error) => {
                            self.disable_db(&format!("delete all events failed: {error}"))
                        }
                    }
                }
            }

            return self
                .delete_from_history_filtered(username, None, None, None, None, None)
                .await;
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
                match self
                    .query_with_deadline(event_bus_db::delete_events_from_db_with_filters(
                        pool,
                        filters.to_db_query(username),
                    ))
                    .await
                {
                    Ok(deleted) => {
                        self.delete_from_history_filtered(
                            username,
                            category,
                            severity,
                            since_minutes,
                            start,
                            end,
                        )
                        .await;
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
}
