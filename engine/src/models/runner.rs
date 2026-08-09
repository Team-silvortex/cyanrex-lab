use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RunnerStatus {
    pub mode: String,
    pub isolation: String,
    pub instance_id: String,
    pub max_concurrent: usize,
    pub max_per_user: usize,
    pub active_total: usize,
    pub active_for_current_user: usize,
    pub available_slots: usize,
    pub execution_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunnerLeaseView {
    pub runner_id: String,
    pub username: String,
    pub runtime_backend: String,
    pub started_at: DateTime<Utc>,
    pub deadline: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunnerOverview {
    pub status: RunnerStatus,
    pub active_leases: Vec<RunnerLeaseView>,
}
