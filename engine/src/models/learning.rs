use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabDefinition {
    pub id: String,
    pub position: u8,
    pub title: String,
    pub summary: String,
    pub doc_slug: String,
    pub template_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LabProgressStatus {
    NotStarted,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabAttempt {
    pub id: String,
    pub username: String,
    pub lab_id: String,
    pub template_id: Option<String>,
    pub source: String,
    pub source_sha256: String,
    pub run_success: bool,
    pub stage: String,
    pub attach_expected: bool,
    pub attach_verified: bool,
    pub completed: bool,
    pub feedback: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LabProgress {
    pub lab: LabDefinition,
    pub status: LabProgressStatus,
    pub attempts: u32,
    pub latest_stage: Option<String>,
    pub latest_feedback: Vec<String>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StudentLearningOverview {
    pub username: String,
    pub completed_labs: u32,
    pub total_labs: u32,
    pub total_attempts: u32,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub labs: Vec<LabProgress>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TeacherLearningOverview {
    pub generated_at: DateTime<Utc>,
    pub total_labs: u32,
    pub active_students: u32,
    pub students: Vec<StudentLearningOverview>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TeacherStudentAttempts {
    pub username: String,
    pub attempts: Vec<LabAttempt>,
}
