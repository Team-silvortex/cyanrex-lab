use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::sqlx_compat as sqlx;
use crate::sqlx_compat::{PgPool, PgPoolOptions, Row};
use chrono::Utc;
use tokio::sync::{OnceCell, RwLock};
use uuid::Uuid;

mod local;
#[cfg(test)]
mod tests;

use crate::config::runtime_instance_id;
use crate::models::script::UserScript;

#[derive(Clone)]
pub struct ScriptStore {
    in_memory: Arc<RwLock<HashMap<String, Vec<UserScript>>>>,
    data_dir: PathBuf,
    db_pool: Option<PgPool>,
    schema_ready: Arc<OnceCell<()>>,
    db_disabled: Arc<AtomicBool>,
    local_writer: Arc<tokio::sync::Mutex<()>>,
}

impl Default for ScriptStore {
    fn default() -> Self {
        let root = std::env::var("CYANREX_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data"));
        let instance_id = runtime_instance_id();
        let data_dir = if instance_id == "default" {
            root.join("scripts")
        } else {
            root.join("scripts").join(instance_id)
        };
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
            data_dir,
            db_pool,
            schema_ready: Arc::new(OnceCell::new()),
            db_disabled: Arc::new(AtomicBool::new(false)),
            local_writer: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

impl ScriptStore {
    pub async fn list_for_user(&self, username: &str) -> Vec<UserScript> {
        let Some(username) = Self::sanitize_script_username(username) else {
            return Vec::new();
        };

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                match sqlx::query(
                    "SELECT id, username, title, script, created_at, updated_at
                     FROM user_scripts
                     WHERE username = $1
                     ORDER BY updated_at DESC",
                )
                .bind(&username)
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

        self.read_local_scripts(&username)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!("local script read failed: {error}");
                Vec::new()
            })
    }

    pub async fn save_for_user(
        &self,
        username: &str,
        title: &str,
        script: &str,
    ) -> Result<UserScript, String> {
        let username = Self::sanitize_script_username(username)
            .ok_or_else(|| "invalid username".to_string())?;

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

        self.commit_local_scripts(&username, local::Mutation::Save(record.clone()))
            .await?;
        Ok(record)
    }

    pub async fn delete_for_user(&self, username: &str, id: &str) -> Result<(), String> {
        let username = Self::sanitize_script_username(username)
            .ok_or_else(|| "invalid username".to_string())?;

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                if let Err(error) =
                    sqlx::query("DELETE FROM user_scripts WHERE id = $1 AND username = $2")
                        .bind(id)
                        .bind(&username)
                        .execute(pool)
                        .await
                {
                    self.disable_db(&format!("delete script failed: {error}"));
                } else {
                    return Ok(());
                }
            }
        }

        self.commit_local_scripts(&username, local::Mutation::Delete(id.to_string()))
            .await
            .map(|_| ())
    }

    fn active_pool(&self) -> Option<&PgPool> {
        if !crate::config::db_fallback_enabled() {
            return None;
        }
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

    fn sanitize_script_username(username: &str) -> Option<String> {
        let sanitized = username
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
            .collect::<String>();
        if sanitized.is_empty() {
            return None;
        }

        if sanitized.len() != username.len() || sanitized.len() > 64 {
            None
        } else {
            Some(sanitized)
        }
    }
}
