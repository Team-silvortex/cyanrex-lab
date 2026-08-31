use crate::sqlx_compat as sqlx;

use super::EventBus;

impl EventBus {
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

                sqlx::query(
                    "CREATE INDEX IF NOT EXISTS idx_event_records_user_time ON event_records(username, timestamp)",
                )
                .execute(pool)
                .await?;
                sqlx::query(
                    "CREATE INDEX IF NOT EXISTS idx_event_records_user_unread ON event_records(username, is_read)",
                )
                .execute(pool)
                .await?;
                sqlx::query(
                    "CREATE INDEX IF NOT EXISTS idx_event_records_user_category_time ON event_records(username, category, timestamp)",
                )
                .execute(pool)
                .await?;
                sqlx::query(
                    "CREATE INDEX IF NOT EXISTS idx_event_records_user_severity_time ON event_records(username, severity, timestamp)",
                )
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
