use super::*;
use crate::services::event_bus_policy::policy_to_str;

impl EventBus {
    /// HTTP reads must not substitute the runtime policy cache for configured storage.
    pub(crate) async fn read_settings_for_user(
        &self,
        username: &str,
    ) -> Result<UserEventSettings, EventReadError> {
        let pool = self.authoritative_pool()?;
        self.read_with_deadline(pool.is_some(), async {
            let Some(pool) = pool else {
                return Ok(self
                    .settings
                    .read()
                    .await
                    .get(username)
                    .copied()
                    .unwrap_or_default());
            };
            self.ensure_schema_on(pool).await?;
            let row = sqlx::query(
                "SELECT max_records, overflow_policy FROM event_user_settings WHERE username = $1",
            )
            .bind(username)
            // A repaired column type must not leave a stale prepared result description.
            .persistent(false)
            .fetch_optional(pool)
            .await?;
            let Some(row) = row else {
                return Ok(UserEventSettings::default());
            };
            let max_records: i64 = row.try_get("max_records")?;
            let policy: String = row.try_get("overflow_policy")?;
            let overflow_policy = if policy.eq_ignore_ascii_case("drop_new") {
                EventOverflowPolicy::DropNew
            } else if policy.eq_ignore_ascii_case("drop_oldest") {
                EventOverflowPolicy::DropOldest
            } else {
                return Err(invalid_stored_settings());
            };
            if !(50..=50000).contains(&max_records) {
                return Err(invalid_stored_settings());
            }
            // Read-only: a concurrent settings update owns runtime cache publication.
            Ok(UserEventSettings {
                max_records: max_records as usize,
                overflow_policy,
            })
        })
        .await
    }

    pub(crate) async fn update_settings_confirmed(
        &self,
        username: &str,
        max_records: usize,
        overflow_policy: EventOverflowPolicy,
    ) -> Result<UserEventSettings, EventMutationError> {
        let username = username.to_owned();
        let settings = UserEventSettings {
            max_records: max_records.clamp(50, 50000),
            overflow_policy,
        };
        self.confirmed_mutation(username.clone(), move |bus, pool| async move {
            if let Some(pool) = pool.as_ref() {
                bus.query_with_deadline(bus.persist_settings_and_trim(pool, &username, settings))
                    .await
                    .map_err(confirmed_mutations::storage_error)?;
            }
            bus.publish_settings_update(&username, settings).await;
            Ok(settings)
        })
        .await
    }

    pub(super) async fn persist_settings_and_trim(
        &self,
        pool: &PgPool,
        username: &str,
        settings: UserEventSettings,
    ) -> Result<(), sqlx::Error> {
        self.ensure_schema_on(pool).await?;
        let mut transaction = pool.begin().await?;
        sqlx::query("INSERT INTO event_user_settings (username, max_records, overflow_policy)
            VALUES ($1, $2, $3) ON CONFLICT (username)
            DO UPDATE SET max_records = EXCLUDED.max_records, overflow_policy = EXCLUDED.overflow_policy")
            .bind(username).bind(settings.max_records as i64).bind(policy_to_str(settings.overflow_policy))
            .execute(&mut *transaction).await?;
        sqlx::query("DELETE FROM event_records WHERE id IN (
            SELECT id FROM event_records WHERE username = $1 ORDER BY timestamp DESC, id DESC OFFSET $2)")
            .bind(username).bind(settings.max_records as i64).execute(&mut *transaction).await?;
        transaction.commit().await
    }

    pub(super) async fn publish_settings_update(
        &self,
        username: &str,
        settings: UserEventSettings,
    ) {
        // Obtain every lock before changing anything. The memory-only path is cancellable;
        // an admitted SQL worker instead retains owner admission until publication completes.
        let mut cache = self.settings.write().await;
        let mut history = self.history.write().await;
        let mut unread = self.unread.write().await;
        let mut capacity = self.drop_new_count_cache.write().await;
        let mut admitted = self.drop_new_admitted.write().await;
        cache.insert(username.to_owned(), settings);
        if let Some(bucket) = history.get_mut(username) {
            let remove = bucket.len().saturating_sub(settings.max_records);
            bucket.drain(..remove);
        }
        if let Some(counter) = unread.get_mut(username) {
            *counter = (*counter).min(settings.max_records);
        }
        capacity.remove(username);
        admitted.remove(username);
    }
}

fn invalid_stored_settings() -> sqlx::Error {
    sqlx::Error::Decode(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "invalid stored event settings",
    )))
}
