use std::{
    collections::HashMap,
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
use crate::models::learning::{
    LabAttempt, LabProgress, LabProgressStatus, StudentLearningOverview, TeacherLearningOverview,
};
use crate::services::learning_catalog::{assess_lab_run, find_lab, lab_definitions};
use crate::sqlx_compat as sqlx;
use crate::sqlx_compat::{PgPool, PgPoolOptions, Row};

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
    in_memory: Arc<RwLock<Vec<LabAttempt>>>,
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
            in_memory: Arc::new(RwLock::new(Vec::new())),
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
        self.in_memory.write().await.push(attempt.clone());
        self.persist_memory().await?;
        Ok(attempt)
    }

    pub async fn attempts_for_user(&self, username: &str) -> Vec<LabAttempt> {
        let Some(username) = sanitize_username(username) else {
            return Vec::new();
        };
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                match sqlx::query(
                    "SELECT * FROM learning_attempts WHERE username = $1 ORDER BY created_at DESC",
                )
                .bind(&username)
                .fetch_all(pool)
                .await
                {
                    Ok(rows) => return rows.into_iter().map(decode_attempt).collect(),
                    Err(error) => {
                        self.disable_db(&format!("list learning attempts failed: {error}"))
                    }
                }
            }
        }

        let _ = self.load_memory().await;
        let mut attempts = self
            .in_memory
            .read()
            .await
            .iter()
            .filter(|attempt| attempt.username == username)
            .cloned()
            .collect::<Vec<_>>();
        attempts.sort_by_key(|attempt| std::cmp::Reverse(attempt.created_at));
        attempts
    }

    pub async fn progress_for_user(&self, username: &str) -> Vec<LabProgress> {
        progress_from_attempts(&self.attempts_for_user(username).await)
    }

    pub async fn teacher_overview(&self) -> TeacherLearningOverview {
        let attempts = self.all_attempts().await;
        let mut grouped: HashMap<String, Vec<LabAttempt>> = HashMap::new();
        for attempt in attempts {
            grouped
                .entry(attempt.username.clone())
                .or_default()
                .push(attempt);
        }

        let total_labs = lab_definitions().len() as u32;
        let mut students = grouped
            .into_iter()
            .map(|(username, attempts)| student_overview(username, &attempts, total_labs))
            .collect::<Vec<_>>();
        students.sort_by(|left, right| {
            right
                .last_activity_at
                .cmp(&left.last_activity_at)
                .then_with(|| left.username.cmp(&right.username))
        });

        TeacherLearningOverview {
            generated_at: Utc::now(),
            total_labs,
            active_students: students.len() as u32,
            students,
        }
    }

    async fn all_attempts(&self) -> Vec<LabAttempt> {
        if let Some(pool) = self.active_pool() {
            if self.ensure_schema().await.is_ok() {
                match sqlx::query("SELECT * FROM learning_attempts ORDER BY created_at DESC")
                    .fetch_all(pool)
                    .await
                {
                    Ok(rows) => return rows.into_iter().map(decode_attempt).collect(),
                    Err(error) => self.disable_db(&format!("learning overview failed: {error}")),
                }
            }
        }
        let _ = self.load_memory().await;
        self.in_memory.read().await.clone()
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

    async fn load_memory(&self) -> Result<(), String> {
        let path = self.data_path.clone();
        let memory = self.in_memory.clone();
        self.memory_loaded
            .get_or_try_init(|| async move {
                if !path.exists() {
                    return Ok::<(), String>(());
                }
                let content = tokio::fs::read_to_string(&path)
                    .await
                    .map_err(|error| format!("failed to read learning attempts: {error}"))?;
                let attempts = serde_json::from_str::<Vec<LabAttempt>>(&content)
                    .map_err(|error| format!("failed to parse learning attempts: {error}"))?;
                *memory.write().await = attempts;
                Ok(())
            })
            .await
            .map(|_| ())
    }

    async fn persist_memory(&self) -> Result<(), String> {
        let _guard = self.persist_lock.lock().await;
        let parent = self
            .data_path
            .parent()
            .ok_or_else(|| "invalid learning data path".to_string())?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("failed to prepare learning data dir: {error}"))?;
        let content = serde_json::to_string_pretty(&*self.in_memory.read().await)
            .map_err(|error| format!("failed to encode learning attempts: {error}"))?;
        let temp_path = self.data_path.with_extension("json.tmp");
        tokio::fs::write(&temp_path, content)
            .await
            .map_err(|error| format!("failed to persist learning attempts: {error}"))?;
        tokio::fs::rename(&temp_path, &self.data_path)
            .await
            .map_err(|error| format!("failed to finalize learning attempts: {error}"))
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
        created_at: row.get("created_at"),
    }
}

fn progress_from_attempts(attempts: &[LabAttempt]) -> Vec<LabProgress> {
    lab_definitions()
        .into_iter()
        .map(|lab| {
            let mut matching = attempts
                .iter()
                .filter(|attempt| attempt.lab_id == lab.id)
                .collect::<Vec<_>>();
            matching.sort_by_key(|attempt| std::cmp::Reverse(attempt.created_at));
            let latest = matching.first().copied();
            let completed_at = matching
                .iter()
                .filter(|attempt| attempt.completed)
                .map(|attempt| attempt.created_at)
                .min();
            let status = if completed_at.is_some() {
                LabProgressStatus::Completed
            } else if matching.is_empty() {
                LabProgressStatus::NotStarted
            } else {
                LabProgressStatus::InProgress
            };
            LabProgress {
                lab,
                status,
                attempts: matching.len() as u32,
                latest_stage: latest.map(|attempt| attempt.stage.clone()),
                latest_feedback: latest
                    .map(|attempt| attempt.feedback.clone())
                    .unwrap_or_default(),
                last_attempt_at: latest.map(|attempt| attempt.created_at),
                completed_at,
            }
        })
        .collect()
}

fn student_overview(
    username: String,
    attempts: &[LabAttempt],
    total_labs: u32,
) -> StudentLearningOverview {
    let labs = progress_from_attempts(attempts);
    StudentLearningOverview {
        username,
        completed_labs: labs
            .iter()
            .filter(|lab| lab.status == LabProgressStatus::Completed)
            .count() as u32,
        total_labs,
        total_attempts: attempts.len() as u32,
        last_activity_at: attempts.iter().map(|attempt| attempt.created_at).max(),
        labs,
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
