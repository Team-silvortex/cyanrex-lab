use super::{sanitize_username, sqlx, LearningStore, Row, Utc, Uuid};
use crate::models::learning::{LabTeacherFeedback, SaveTeacherFeedbackRequest};

#[cfg(test)]
#[path = "feedback_tests.rs"]
mod tests;

#[derive(Debug)]
pub enum LearningFeedbackError {
    InvalidInput(&'static str),
    NotFound,
    Conflict,
    Storage(String),
}

impl LearningStore {
    pub async fn save_teacher_feedback(
        &self,
        reviewer: &str,
        request: &SaveTeacherFeedbackRequest,
    ) -> Result<LabTeacherFeedback, LearningFeedbackError> {
        use LearningFeedbackError::*;
        if sanitize_username(&request.username).is_none()
            || sanitize_username(reviewer).is_none()
            || Uuid::parse_str(&request.attempt_id).is_err()
        {
            return Err(InvalidInput("invalid username or attempt id"));
        }
        let comment = request.comment.trim();
        if comment.is_empty() || comment.chars().count() > 2000 {
            return Err(InvalidInput("comment must contain 1 to 2000 characters"));
        }
        let feedback = LabTeacherFeedback {
            reviewer: reviewer.to_string(),
            comment: comment.to_string(),
            revision: request
                .expected_revision
                .checked_add(1)
                .ok_or(InvalidInput("feedback revision limit reached"))?,
            updated_at: Utc::now(),
        };

        if let Some(pool) = self.active_pool() {
            // Do not fall back on a failed feedback write: that could overwrite a stale local copy.
            self.ensure_schema()
                .await
                .map_err(|error| Storage(error.to_string()))?;
            let saved = sqlx::query(
                "UPDATE learning_attempts SET teacher_feedback_reviewer = $1,
                 teacher_feedback_comment = $2, teacher_feedback_revision = $3,
                 teacher_feedback_updated_at = $4
                 WHERE id = $5 AND username = $6 AND teacher_feedback_revision = $7
                 RETURNING id",
            )
            .bind(&feedback.reviewer)
            .bind(&feedback.comment)
            .bind(i64::from(feedback.revision))
            .bind(feedback.updated_at)
            .bind(&request.attempt_id)
            .bind(&request.username)
            .bind(i64::from(request.expected_revision))
            .fetch_optional(pool)
            .await
            .map_err(|error| Storage(error.to_string()))?;
            if saved.is_some() {
                return Ok(feedback);
            }
            let exists =
                sqlx::query("SELECT id FROM learning_attempts WHERE id = $1 AND username = $2")
                    .bind(&request.attempt_id)
                    .bind(&request.username)
                    .fetch_optional(pool)
                    .await
                    .map_err(|error| Storage(error.to_string()))?;
            return Err(if exists.is_some() { Conflict } else { NotFound });
        }

        self.load_memory().await.map_err(Storage)?;
        let _guard = self.persist_lock.lock().await;
        let mut attempts = self.in_memory.read().await.clone();
        let attempt = attempts
            .iter_mut()
            .find(|attempt| {
                attempt.id == request.attempt_id && attempt.username == request.username
            })
            .ok_or(NotFound)?;
        let revision = attempt
            .teacher_feedback
            .as_ref()
            .map_or(0, |value| value.revision);
        if revision != request.expected_revision {
            return Err(Conflict);
        }
        attempt.teacher_feedback = Some(feedback.clone());
        self.persist_attempts(&attempts).await.map_err(Storage)?;
        *self.in_memory.write().await = attempts;
        Ok(feedback)
    }
}

pub(super) fn decode_feedback(
    row: &crate::sqlx_compat::postgres::PgRow,
) -> Option<LabTeacherFeedback> {
    let revision: i64 = row.get("teacher_feedback_revision");
    if revision == 0 {
        return None;
    }
    Some(LabTeacherFeedback {
        reviewer: row.get::<Option<String>, _>("teacher_feedback_reviewer")?,
        comment: row.get::<Option<String>, _>("teacher_feedback_comment")?,
        revision: u32::try_from(revision).ok()?,
        updated_at: row.get::<Option<chrono::DateTime<Utc>>, _>("teacher_feedback_updated_at")?,
    })
}
