use super::*;

impl EventBus {
    // Caller holds this owner's publication/mutation admission. A cold owner has no earlier
    // pending publications; mutations invalidate only after their persistence barrier completes.
    pub(super) async fn drop_new_admission_count(&self, username: &str) -> Option<usize> {
        let pool = self.active_pool()?;
        if let Some(count) = self.drop_new_admitted.read().await.get(username) {
            return Some(*count);
        }
        let result = self
            .query_with_deadline(async {
                self.ensure_schema().await?;
                sqlx::query("SELECT COUNT(*) AS count FROM event_records WHERE username = $1")
                    .bind(username)
                    .fetch_one(pool)
                    .await
            })
            .await;
        match result {
            Ok(row) => {
                let count: i64 = row.get("count");
                let count = count.max(0) as usize;
                self.drop_new_admitted
                    .write()
                    .await
                    .insert(username.to_owned(), count);
                Some(count)
            }
            Err(error) => {
                // Keep the existing bounded volatile fallback, never pretend it is durable.
                self.disable_db(&format!("load drop-new admission capacity failed: {error}"));
                None
            }
        }
    }
}
