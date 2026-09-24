use super::*;
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventMutationError {
    InvalidFilters,
    Unconfirmed,
}

pub(super) fn storage_error(error: sqlx::Error) -> EventMutationError {
    tracing::warn!(%error, "event mutation could not be confirmed");
    EventMutationError::Unconfirmed
}

impl EventBus {
    // HTTP acknowledgements must not inherit the legacy helpers' volatile fallback contract.
    pub(super) async fn confirmed_mutation<T, F, Fut>(
        &self,
        username: String,
        action: F,
    ) -> Result<T, EventMutationError>
    where
        T: Send + 'static,
        F: FnOnce(EventBus, Option<PgPool>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, EventMutationError>> + Send + 'static,
    {
        let waiting = async {
            let guard = self.mutation_admission(&username).await;
            let pool = self
                .authoritative_pool()
                .map_err(|_| EventMutationError::Unconfirmed)?
                .cloned();
            let Some(pool) = pool else {
                return action(self.clone(), None).await;
            };
            let bus = self.clone();
            // Dropping the caller or expiring its wait is not rollback. The admitted SQL worker
            // keeps owner admission through storage and memory publication, as legacy mutations do.
            tokio::spawn(async move {
                let _guard = guard;
                bus.flush_persistence().await;
                bus.authoritative_pool()
                    .map_err(|_| EventMutationError::Unconfirmed)?;
                let result = action(bus.clone(), Some(pool)).await?;
                bus.authoritative_pool()
                    .map_err(|_| EventMutationError::Unconfirmed)?;
                Ok(result)
            })
            .await
            .map_err(|error| {
                tracing::warn!(%error, "event mutation worker did not confirm completion");
                EventMutationError::Unconfirmed
            })?
        };
        tokio::time::timeout(StdDuration::from_secs(10), waiting)
            .await
            .unwrap_or_else(|_| {
                tracing::warn!(
                    "event mutation caller deadline exceeded; completion is unconfirmed"
                );
                Err(EventMutationError::Unconfirmed)
            })
    }

    pub(crate) async fn acknowledge_all_read_for_user(
        &self,
        username: &str,
    ) -> Result<(), EventMutationError> {
        let username = username.to_owned();
        self.confirmed_mutation(username.clone(), move |bus, pool| async move {
            if let Some(pool) = pool.as_ref() {
                bus.query_with_deadline(async {
                    bus.ensure_schema_on(pool).await?;
                    sqlx::query("UPDATE event_records SET is_read = true WHERE username = $1 AND is_read = false")
                        .bind(&username).execute(pool).await.map(|_| ())
                }).await.map_err(storage_error)?;
            }
            let mut unread = bus.unread.write().await;
            if pool.is_some() { bus.authoritative_pool().map_err(|_| EventMutationError::Unconfirmed)?; }
            unread.insert(username, 0);
            Ok(())
        }).await
    }

    pub(crate) async fn delete_user_events_confirmed(
        &self,
        username: &str,
        filters: EventQueryFilters<'_>,
    ) -> Result<usize, EventMutationError> {
        // Validate/freeze before admission: waiting must not broaden the reviewed time window.
        let filters = filters
            .checked_for_read()
            .map_err(|_| EventMutationError::InvalidFilters)?;
        let username = username.to_owned();
        let category = filters.category.map(str::to_owned);
        let severity = filters.severity.map(str::to_owned);
        let (start, end) = (filters.start, filters.end);
        self.confirmed_mutation(username.clone(), move |bus, pool| async move {
            let filters = EventQueryFilters {
                category: category.as_deref(),
                severity: severity.as_deref(),
                start,
                end,
                ..Default::default()
            };
            let durable_deleted = if let Some(pool) = pool.as_ref() {
                Some(
                    bus.query_with_deadline(async {
                        bus.ensure_schema_on(pool).await?;
                        event_bus_db::delete_events_from_db_with_filters(
                            pool,
                            filters.to_db_query(&username),
                        )
                        .await
                    })
                    .await
                    .map_err(storage_error)? as usize,
                )
            } else {
                None
            };
            let memory_deleted = bus
                .delete_from_history_filtered(
                    &username,
                    filters.category,
                    filters.severity,
                    None,
                    start,
                    end,
                )
                .await;
            Ok(durable_deleted.unwrap_or(memory_deleted))
        })
        .await
    }
}
