use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use chrono::Utc;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use tokio::sync::{OnceCell, RwLock};
use uuid::Uuid;

use crate::models::script::UserScript;

#[derive(Clone)]
pub struct ScriptStore {
    in_memory: Arc<RwLock<HashMap<String, Vec<UserScript>>>>,
    data_dir: PathBuf,
    db_pool: Option<PgPool>,
    schema_ready: Arc<OnceCell<()>>,
    db_disabled: Arc<AtomicBool>,
}

impl Default for ScriptStore {
    fn default() -> Self {
        let root = std::env::var("CYANREX_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data"));
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
            in_memory: Arc::new(RwLock::new(HashMap::new())),
            data_dir: root.join("scripts"),
            db_pool,
            schema_ready: Arc::new(OnceCell::new()),
            db_disabled: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl ScriptStore {
    pub async fn list_for_user(&self, username: &str) -> Vec<UserScript> {
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                match sqlx::query(
                    "SELECT id, username, title, script, created_at, updated_at
                     FROM user_scripts
                     WHERE username = $1
                     ORDER BY updated_at DESC",
                )
                .bind(username)
                .fetch_all(pool)
                .await
                {
                    Ok(rows) => {
                        return rows
                            .into_iter()
                            .map(|row| UserScript {
                                id: row.get("id"),
                                username: row.get("username"),
                                title: row.get("title"),
                                script: row.get("script"),
                                created_at: row.get("created_at"),
                                updated_at: row.get("updated_at"),
                            })
                            .collect();
                    }
                    Err(error) => {
                        self.disable_db(&format!("list scripts failed: {error}"));
                    }
                }
            }
        }

        let _ = self.load_memory_user(username).await;
        let cache = self.in_memory.read().await;
        cache.get(username).cloned().unwrap_or_default()
    }

    pub async fn save_for_user(
        &self,
        username: &str,
        title: &str,
        script: &str,
    ) -> Result<UserScript, String> {
        let clean_title = title.trim();
        if clean_title.is_empty() {
            return Err("title is required".to_string());
        }
        if script.trim().is_empty() {
            return Err("script content is empty".to_string());
        }

        let now = Utc::now();
        let id = Uuid::new_v4().to_string();
        let record = UserScript {
            id: id.clone(),
            username: username.to_string(),
            title: clean_title.to_string(),
            script: script.to_string(),
            created_at: now,
            updated_at: now,
        };

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                if let Err(error) = sqlx::query(
                    "INSERT INTO user_scripts (id, username, title, script, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(&record.id)
                .bind(&record.username)
                .bind(&record.title)
                .bind(&record.script)
                .bind(record.created_at)
                .bind(record.updated_at)
                .execute(pool)
                .await
                {
                    self.disable_db(&format!("save script failed: {error}"));
                } else {
                    return Ok(record);
                }
            }
        }

        self.ensure_data_dir().await?;
        let _ = self.load_memory_user(username).await;
        {
            let mut cache = self.in_memory.write().await;
            let bucket = cache.entry(username.to_string()).or_default();
            bucket.insert(0, record.clone());
        }
        self.persist_memory_user(username).await?;
        Ok(record)
    }

    pub async fn delete_for_user(&self, username: &str, id: &str) -> Result<(), String> {
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                if let Err(error) =
                    sqlx::query("DELETE FROM user_scripts WHERE id = $1 AND username = $2")
                        .bind(id)
                        .bind(username)
                        .execute(pool)
                        .await
                {
                    self.disable_db(&format!("delete script failed: {error}"));
                } else {
                    return Ok(());
                }
            }
        }

        self.ensure_data_dir().await?;
        let _ = self.load_memory_user(username).await;
        {
            let mut cache = self.in_memory.write().await;
            if let Some(bucket) = cache.get_mut(username) {
                bucket.retain(|script| script.id != id);
            }
        }
        self.persist_memory_user(username).await
    }

    fn active_pool(&self) -> Option<&PgPool> {
        if self.db_disabled.load(Ordering::Relaxed) {
            return None;
        }
        self.db_pool.as_ref()
    }

    fn disable_db(&self, reason: &str) {
        if !self.db_disabled.swap(true, Ordering::Relaxed) {
            tracing::warn!("script store db disabled, fallback to local file: {reason}");
        }
    }

    async fn ensure_schema(&self) -> Result<(), sqlx::Error> {
        let Some(pool) = self.active_pool() else {
            return Ok(());
        };

        self.schema_ready
            .get_or_try_init(|| async move {
                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS user_scripts (
                        id TEXT PRIMARY KEY,
                        username TEXT NOT NULL,
                        title TEXT NOT NULL,
                        script TEXT NOT NULL,
                        created_at TIMESTAMPTZ NOT NULL,
                        updated_at TIMESTAMPTZ NOT NULL
                    )",
                )
                .execute(pool)
                .await?;

                sqlx::query(
                    "CREATE INDEX IF NOT EXISTS idx_user_scripts_username_updated
                     ON user_scripts(username, updated_at DESC)",
                )
                .execute(pool)
                .await?;

                Ok::<(), sqlx::Error>(())
            })
            .await
            .map(|_| ())
    }

    async fn ensure_data_dir(&self) -> Result<(), String> {
        tokio::fs::create_dir_all(&self.data_dir)
            .await
            .map_err(|error| format!("failed to prepare script data dir: {error}"))
    }

    async fn persist_memory_user(&self, username: &str) -> Result<(), String> {
        let path = self.data_dir.join(format!("{username}.json"));
        let bucket = {
            let cache = self.in_memory.read().await;
            cache.get(username).cloned().unwrap_or_default()
        };
        let content = serde_json::to_string_pretty(&bucket)
            .map_err(|error| format!("failed to encode scripts: {error}"))?;
        tokio::fs::write(path, content)
            .await
            .map_err(|error| format!("failed to persist scripts: {error}"))
    }

    async fn load_memory_user(&self, username: &str) -> Result<(), String> {
        {
            let cache = self.in_memory.read().await;
            if cache.contains_key(username) {
                return Ok(());
            }
        }

        let path = self.data_dir.join(format!("{username}.json"));
        if !path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|error| format!("failed to read persisted scripts: {error}"))?;
        let records = serde_json::from_str::<Vec<UserScript>>(&content)
            .map_err(|error| format!("failed to parse persisted scripts: {error}"))?;

        let mut cache = self.in_memory.write().await;
        cache.insert(username.to_string(), records);
        Ok(())
    }
}
