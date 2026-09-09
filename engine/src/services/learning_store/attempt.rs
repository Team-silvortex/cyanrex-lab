use super::{decode_attempt, sanitize_username, sqlx, Arc, LabAttempt, LearningStore};

impl LearningStore {
    pub async fn attempt_for_user(
        &self,
        username: &str,
        attempt_id: &str,
    ) -> Result<Option<LabAttempt>, String> {
        let Some(username) = sanitize_username(username) else {
            return Ok(None);
        };
        if let Some(pool) = self.active_pool() {
            // A resume must report an unavailable record store, not silently serve an older fallback.
            self.ensure_schema()
                .await
                .map_err(|error| error.to_string())?;
            return sqlx::query("SELECT * FROM learning_attempts WHERE username = $1 AND id = $2")
                .bind(&username)
                .bind(attempt_id)
                .fetch_optional(pool)
                .await
                .map(|row| row.map(decode_attempt))
                .map_err(|error| error.to_string());
        }
        self.load_memory().await?;
        let snapshot = self.in_memory.read().await.clone();
        Ok(snapshot
            .iter()
            .map(Arc::as_ref)
            .find(|row| row.username == username && row.id == attempt_id)
            .cloned())
    }
}
