use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunnerAgentIsolation {
    SharedKernel,
    Container,
    VirtualMachine,
    DedicatedHost,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunnerAgentState {
    Healthy,
    Degraded,
    Draining,
    Offline,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerAgentRegisterRequest {
    pub agent_id: String,
    pub protocol_version: u16,
    pub agent_version: String,
    pub isolation: RunnerAgentIsolation,
    pub max_concurrent: u16,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerAgentHeartbeatRequest {
    pub agent_id: String,
    pub state: RunnerAgentState,
    pub active_jobs: u16,
    pub available_slots: u16,
    pub kernel_release: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerAgentView {
    pub agent_id: String,
    pub protocol_version: u16,
    pub agent_version: String,
    pub isolation: RunnerAgentIsolation,
    pub state: RunnerAgentState,
    pub max_concurrent: u16,
    pub active_jobs: u16,
    pub available_slots: u16,
    pub capabilities: Vec<String>,
    pub labels: BTreeMap<String, String>,
    pub kernel_release: Option<String>,
    pub message: Option<String>,
    pub registered_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerAgentInventory {
    pub generated_at: DateTime<Utc>,
    pub enabled: bool,
    pub total_agents: usize,
    pub online_agents: usize,
    pub agents: Vec<RunnerAgentView>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RunnerAgentRegistrationResponse {
    #[serde(flatten)]
    pub agent: RunnerAgentView,
    pub credential: String,
    pub signature_scheme: String,
}
