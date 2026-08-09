use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunnerJobState {
    Queued,
    Claimed,
    CancelRequested,
    Succeeded,
    Failed,
    Cancelled,
    Expired,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunnerJobResultState {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunnerProbeSubmitRequest {
    pub agent_id: Option<String>,
    pub message: String,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunnerCompileCheckSubmitRequest {
    pub agent_id: Option<String>,
    pub source: String,
    pub program_name: Option<String>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerCompileReport {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stdout_truncated: bool,
    pub stderr: String,
    pub stderr_truncated: bool,
    pub object_bytes: Option<u64>,
    pub object_sha256: Option<String>,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunnerJobCancelRequest {
    pub job_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerJobClaimRequest {
    pub agent_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerJobLeaseReference {
    pub job_id: String,
    pub lease_token: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerJobSyncRequest {
    pub agent_id: String,
    #[serde(default)]
    pub leases: Vec<RunnerJobLeaseReference>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerJobResultRequest {
    pub agent_id: String,
    pub job_id: String,
    pub lease_token: String,
    pub state: RunnerJobResultState,
    pub message: Option<String>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerJobClaim {
    pub job_id: String,
    pub kind: String,
    pub message: String,
    pub source: Option<String>,
    pub program_name: Option<String>,
    pub lease_token: String,
    pub claimed_at: DateTime<Utc>,
    pub deadline: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerJobClaimResponse {
    pub job: Option<RunnerJobClaim>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerJobSyncResponse {
    pub cancel_job_ids: Vec<String>,
    pub lost_job_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerJobView {
    pub job_id: String,
    pub kind: String,
    pub state: RunnerJobState,
    pub target_agent_id: Option<String>,
    pub assigned_agent_id: Option<String>,
    pub owner_username: Option<String>,
    pub message: String,
    pub source_bytes: Option<usize>,
    pub program_name: Option<String>,
    pub timeout_seconds: u64,
    pub result_message: Option<String>,
    pub output: Option<String>,
    pub created_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub deadline: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerJobInventory {
    pub generated_at: DateTime<Utc>,
    pub total_jobs: usize,
    pub jobs: Vec<RunnerJobView>,
}
