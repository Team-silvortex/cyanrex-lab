use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use chrono::Utc;
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, OnceCell, RwLock};
use uuid::Uuid;

use crate::config::runtime_instance_id;
use crate::models::learning::{LabAttempt, LabProgress, TeacherLearningOverview};
use crate::services::learning_catalog::{assess_lab_run, find_lab};
use crate::sqlx_compat as sqlx;
use crate::sqlx_compat::{PgPool, PgPoolOptions, Row};

mod attempt;
mod feedback;
mod loading;
mod persistence;
mod queries;
#[cfg(test)]
mod snapshot_tests;
pub use feedback::LearningFeedbackError;
use queries::{progress_from_attempts, select_recent, teacher_overview_from_attempts};

type AttemptSnapshot = Arc<Vec<Arc<LabAttempt>>>;

pub struct LearningRunOutcome<'a> {
    pub lab_id: &'a str,
    pub template_id: Option<&'a str>,
    pub source: &'a str,
    pub run_success: bool,
    pub stage: &'a str,
    pub attach_expected: bool,
    pub attach_verified: bool,
}

#[derive(Clone)]
pub struct LearningStore {
    in_memory: Arc<RwLock<AttemptSnapshot>>,
    data_path: PathBuf,
    persist_lock: Arc<Mutex<()>>,
    memory_loaded: Arc<OnceCell<()>>,
    db_pool: Option<PgPool>,
    schema_ready: Arc<OnceCell<()>>,
    db_disabled: Arc<AtomicBool>,
}

impl Default for LearningStore {
    fn default() -> Self {
        let root = std::env::var("CYANREX_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./data"));
        let data_path = root
            .join("learning")
            .join(runtime_instance_id())
            .join("attempts.json");
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
            in_memory: Arc::new(RwLock::new(Arc::new(Vec::new()))),
            data_path,
            persist_lock: Arc::new(Mutex::new(())),
            memory_loaded: Arc::new(OnceCell::new()),
            db_pool,
            schema_ready: Arc::new(OnceCell::new()),
            db_disabled: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl LearningStore {
    pub fn with_local_data_path(data_path: PathBuf) -> Self {
        Self {
            in_memory: Arc::new(RwLock::new(Arc::new(Vec::new()))),
            data_path,
            persist_lock: Arc::new(Mutex::new(())),
            memory_loaded: Arc::new(OnceCell::new()),
            db_pool: None,
            schema_ready: Arc::new(OnceCell::new()),
            db_disabled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn record_run(
        &self,
        username: &str,
        outcome: LearningRunOutcome<'_>,
    ) -> Result<LabAttempt, String> {
        let username = sanitize_username(username).ok_or_else(|| "invalid username".to_string())?;
        find_lab(outcome.lab_id).ok_or_else(|| format!("unknown lab id: {}", outcome.lab_id))?;
        let assessment = assess_lab_run(
            outcome.lab_id,
            outcome.template_id,
            outcome.source,
            outcome.run_success,
            outcome.stage,
            outcome.attach_expected,
            outcome.attach_verified,
        )?;
        let attempt = LabAttempt {
            id: Uuid::new_v4().to_string(),
            username,
            lab_id: outcome.lab_id.to_string(),
            template_id: outcome.template_id.map(str::to_string),
            source: outcome.source.to_string(),
            source_sha256: format!("{:x}", Sha256::digest(outcome.source.as_bytes())),
            run_success: outcome.run_success,
            stage: outcome.stage.to_string(),
            attach_expected: outcome.attach_expected,
            attach_verified: outcome.attach_verified,
            completed: assessment.completed,
            feedback: assessment.feedback,
            teacher_feedback: None,
            created_at: Utc::now(),
        };

        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                let feedback = attempt.feedback.join("\n");
                let inserted = sqlx::query(
                    "INSERT INTO learning_attempts
                     (id, username, lab_id, template_id, source, source_sha256, run_success,
                      stage, attach_expected, attach_verified, completed, feedback, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
                )
                .bind(&attempt.id)
                .bind(&attempt.username)
                .bind(&attempt.lab_id)
                .bind(&attempt.template_id)
                .bind(&attempt.source)
                .bind(&attempt.source_sha256)
                .bind(attempt.run_success)
                .bind(&attempt.stage)
                .bind(attempt.attach_expected)
                .bind(attempt.attach_verified)
                .bind(attempt.completed)
                .bind(feedback)
                .bind(attempt.created_at)
                .execute(pool)
                .await;
                match inserted {
                    Ok(_) => return Ok(attempt),
                    Err(error) => self.disable_db(&format!("record learning run failed: {error}")),
                }
            }
        }

        self.load_memory().await?;
        let guard = self.persist_lock.clone().lock_owned().await;
        let current = self.in_memory.read().await.clone();
        let mut rows = Vec::with_capacity(current.len() + 1);
        rows.extend(current.iter().cloned());
        rows.push(Arc::new(attempt.clone()));
        let attempts = Arc::new(rows);
        self.persist_and_publish(attempts, guard).await?;
        Ok(attempt)
    }

    pub async fn attempts_for_user(&self, username: &str) -> Result<Vec<LabAttempt>, String> {
        self.attempts_for_user_with_limit(username, None).await
    }

    pub async fn recent_attempts_for_user(
        &self,
        username: &str,
        limit: usize,
    ) -> Result<Vec<LabAttempt>, String> {
        self.attempts_for_user_with_limit(username, Some(limit.clamp(1, 50)))
            .await
    }

    async fn attempts_for_user_with_limit(
        &self,
        username: &str,
        limit: Option<usize>,
    ) -> Result<Vec<LabAttempt>, String> {
        let Some(username) = sanitize_username(username) else {
            return Ok(Vec::new());
        };
        if let Some(pool) = self.active_pool() {
            // As with single-attempt resume, a failed authoritative read is not empty success.
            self.ensure_schema()
                .await
                .map_err(|error| error.to_string())?;
            let fetched = match limit {
                Some(limit) => {
                    sqlx::query(
                        "SELECT * FROM learning_attempts WHERE username = $1
                         ORDER BY created_at DESC LIMIT $2",
                    )
                    .bind(&username)
                    .bind(limit as i64)
                    .fetch_all(pool)
                    .await
                }
                None => sqlx::query(
                    "SELECT * FROM learning_attempts WHERE username = $1 ORDER BY created_at DESC",
                )
                .bind(&username)
                .fetch_all(pool)
                .await,
            };
            return fetched
                .map(|rows| rows.into_iter().map(decode_attempt).collect())
                .map_err(|error| error.to_string());
        }

        self.load_memory().await?;
        let snapshot = self.in_memory.read().await.clone();
        Ok(select_recent(
            snapshot
                .iter()
                .map(Arc::as_ref)
                .filter(|attempt| attempt.username == username),
            limit,
        )
        .into_iter()
        .cloned()
        .collect())
    }

    pub async fn progress_for_user(&self, username: &str) -> Result<Vec<LabProgress>, String> {
        if self.active_pool().is_some() {
            return Ok(progress_from_attempts(
                &self.attempts_for_user(username).await?,
            ));
        }
        let Some(username) = sanitize_username(username) else {
            return Ok(progress_from_attempts(std::iter::empty()));
        };
        self.load_memory().await?;
        let snapshot = self.in_memory.read().await.clone();
        Ok(progress_from_attempts(
            snapshot
                .iter()
                .map(Arc::as_ref)
                .filter(|row| row.username == username),
        ))
    }

    pub async fn teacher_overview(&self) -> Result<TeacherLearningOverview, String> {
        let attempts = self.all_attempts().await?;
        Ok(teacher_overview_from_attempts(
            attempts.iter().map(Arc::as_ref),
        ))
    }

    async fn all_attempts(&self) -> Result<AttemptSnapshot, String> {
        if let Some(pool) = self.active_pool() {
            self.ensure_schema()
                .await
                .map_err(|error| error.to_string())?;
            return sqlx::query("SELECT * FROM learning_attempts ORDER BY created_at DESC")
                .fetch_all(pool)
                .await
                .map(|rows| Arc::new(rows.into_iter().map(decode_attempt).map(Arc::new).collect()))
                .map_err(|error| error.to_string());
        }
        self.load_memory().await?;
        Ok(self.in_memory.read().await.clone())
    }

    fn active_pool(&self) -> Option<&PgPool> {
        if !crate::config::db_fallback_enabled() || self.db_disabled.load(Ordering::Relaxed) {
            return None;
        }
        self.db_pool.as_ref()
    }

    fn disable_db(&self, reason: &str) {
        if !self.db_disabled.swap(true, Ordering::Relaxed) {
            tracing::warn!("learning store db disabled, fallback to local file: {reason}");
        }
    }

    async fn ensure_schema(&self) -> Result<(), sqlx::Error> {
        let Some(pool) = self.active_pool() else {
            return Ok(());
        };
        self.schema_ready
            .get_or_try_init(|| async move {
                sqlx::query(
                    "CREATE TABLE IF NOT EXISTS learning_attempts (
                        id TEXT PRIMARY KEY,
                        username TEXT NOT NULL,
                        lab_id TEXT NOT NULL,
                        template_id TEXT,
                        source TEXT NOT NULL,
                        source_sha256 TEXT NOT NULL,
                        run_success BOOLEAN NOT NULL,
                        stage TEXT NOT NULL,
                        attach_expected BOOLEAN NOT NULL,
                        attach_verified BOOLEAN NOT NULL,
                        completed BOOLEAN NOT NULL,
                        feedback TEXT NOT NULL,
                        created_at TIMESTAMPTZ NOT NULL
                    )",
                )
                .execute(pool)
                .await?;
                sqlx::query(include_str!(
                    "../../migrations/0005_learning_teacher_feedback.sql"
                ))
                .execute(pool)
                .await?;
                sqlx::query(
                    "CREATE INDEX IF NOT EXISTS idx_learning_attempts_user_lab_created
                     ON learning_attempts(username, lab_id, created_at DESC)",
                )
                .execute(pool)
                .await?;
                Ok::<(), sqlx::Error>(())
            })
            .await
            .map(|_| ())
    }
}

fn decode_attempt(row: crate::sqlx_compat::postgres::PgRow) -> LabAttempt {
    let feedback: String = row.get("feedback");
    LabAttempt {
        id: row.get("id"),
        username: row.get("username"),
        lab_id: row.get("lab_id"),
        template_id: row.get("template_id"),
        source: row.get("source"),
        source_sha256: row.get("source_sha256"),
        run_success: row.get("run_success"),
        stage: row.get("stage"),
        attach_expected: row.get("attach_expected"),
        attach_verified: row.get("attach_verified"),
        completed: row.get("completed"),
        feedback: feedback.lines().map(str::to_string).collect(),
        teacher_feedback: feedback::decode_feedback(&row),
        created_at: row.get("created_at"),
    }
}

fn sanitize_username(username: &str) -> Option<String> {
    let sanitized = username
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(*c, '_' | '-' | '.'))
        .collect::<String>();
    if sanitized.is_empty() || sanitized.len() != username.len() || sanitized.len() > 64 {
        None
    } else {
        Some(sanitized)
    }
}

#[cfg(test)]
mod tests {
    use super::{progress_from_attempts, LabAttempt};
    use crate::models::learning::LabProgressStatus;
    use chrono::Utc;

    #[test]
    fn progress_prefers_completed_state_over_later_failures() {
        let base = LabAttempt {
            id: "one".to_string(),
            username: "student".to_string(),
            lab_id: "01-first-program".to_string(),
            template_id: Some("xdp-pass".to_string()),
            source: "source".to_string(),
            source_sha256: "hash".to_string(),
            run_success: true,
            stage: "run".to_string(),
            attach_expected: false,
            attach_verified: false,
            completed: true,
            feedback: vec!["passed".to_string()],
            teacher_feedback: None,
            created_at: Utc::now(),
        };
        let mut later_failure = base.clone();
        later_failure.id = "two".to_string();
        later_failure.completed = false;
        later_failure.created_at += chrono::Duration::seconds(1);
        let progress = progress_from_attempts(&[base, later_failure]);
        assert_eq!(progress[0].status, LabProgressStatus::Completed);
        assert_eq!(progress[0].attempts, 2);
    }
}
