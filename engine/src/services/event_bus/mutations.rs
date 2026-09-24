use super::*;
use crate::services::event_bus_codec::{to_category_str, to_color_str, to_severity_str};
use crate::sqlx_compat::{types::Json, Postgres, QueryBuilder};

impl EventBus {
    /// Legacy best-effort helper; HTTP uses acknowledge_all_read_for_user for confirmed results.
    pub async fn mark_all_read_for_user(&self, username: &str) {
        let username = username.to_owned();
        self.with_persistence_barrier(username.clone(), move |bus| async move {
            if let Some(pool) = bus.active_pool() {
                if bus.ensure_schema().await.is_ok() {
                    if let Err(error) = bus.query_with_deadline(sqlx::query(
                        "UPDATE event_records SET is_read = true WHERE username = $1 AND is_read = false",
                    ).bind(&username).execute(pool)).await {
                        bus.disable_db(&format!("mark read failed: {error}"));
                    }
                } else {
                    bus.disable_db("mark read schema initialization failed");
                }
            }
            bus.unread.write().await.insert(username, 0);
        }).await;
    }

    pub async fn replace_user_events(&self, username: &str, events: Vec<Event>) {
        // The service API must not turn a replacement into foreign-owner insertion or disclosure.
        if events.iter().any(|event| event.username != username) {
            tracing::warn!("rejected event replacement containing a foreign owner");
            return;
        }
        let username = username.to_owned();
        self.with_persistence_barrier(username.clone(), move |bus| async move {
            let max_records = bus.settings_for_user(&username).await.max_records.max(1);
            let mut events = events;
            if events.len() > max_records {
                events.drain(..events.len() - max_records);
            }
            if let Some(pool) = bus.active_pool() {
                if let Err(error) = bus
                    .query_with_deadline(bus.replace_events_in_db(pool, &username, &events))
                    .await
                {
                    bus.disable_db(&format!("replace events failed: {error}"));
                    return;
                }
            }
            let mut history = bus.history.write().await;
            let mut unread = bus.unread.write().await;
            let mut capacity = bus.drop_new_count_cache.write().await;
            let mut admitted = bus.drop_new_admitted.write().await;
            history.insert(username.clone(), events.into());
            unread.insert(username.clone(), 0);
            capacity.remove(&username);
            admitted.remove(&username);
        })
        .await;
    }

    async fn replace_events_in_db(
        &self,
        pool: &PgPool,
        username: &str,
        events: &[Event],
    ) -> Result<(), sqlx::Error> {
        self.ensure_schema().await?;
        let mut transaction = pool.begin().await?;
        sqlx::query("DELETE FROM event_records WHERE username = $1")
            .bind(username)
            .execute(&mut *transaction)
            .await?;
        // Keep PostgreSQL bind counts bounded even at the 50,000-row retention limit.
        for chunk in events.chunks(256) {
            let mut query = QueryBuilder::<Postgres>::new(
                "INSERT INTO event_records (username, timestamp, source, event_type, category, severity, color, payload, is_read) ",
            );
            query.push_values(chunk, |mut builder, event| {
                builder
                    .push_bind(username)
                    .push_bind(event.timestamp)
                    .push_bind(&event.source)
                    .push_bind(&event.event_type)
                    .push_bind(to_category_str(event.category))
                    .push_bind(to_severity_str(event.severity))
                    .push_bind(to_color_str(event.color))
                    .push_bind(Json(&event.payload))
                    .push_bind(true);
            });
            query.build().execute(&mut *transaction).await?;
        }
        transaction.commit().await
    }

    /// Legacy best-effort helper; HTTP uses update_settings_confirmed instead.
    pub async fn update_settings_for_user(
        &self,
        username: &str,
        max_records: usize,
        overflow_policy: EventOverflowPolicy,
    ) -> Result<UserEventSettings, String> {
        let username = username.to_owned();
        let settings = UserEventSettings {
            max_records: max_records.clamp(50, 50000),
            overflow_policy,
        };
        self.with_persistence_barrier(username.clone(), move |bus| async move {
            if let Some(pool) = bus.active_pool() {
                // Persist policy and trim together; an error must not publish a new cache or
                // irreversibly shrink memory while reporting that the settings update failed.
                let result = bus
                    .query_with_deadline(bus.persist_settings_and_trim(pool, &username, settings))
                    .await;
                result.map_err(|error| format!("update event settings failed: {error}"))?;
            }
            bus.publish_settings_update(&username, settings).await;
            Ok(settings)
        })
        .await
    }
}
