use serde::{Deserialize, Serialize};

use super::{
    runner_agent::{RunnerAgentIsolation, RunnerAgentState},
    runner_job::RunnerJobState,
};

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EbpfRuntimeBackend {
    #[default]
    Bpftool,
    Aya,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EbpfRunRequest {
    pub code: String,
    pub template_id: Option<String>,
    pub lab_id: Option<String>,
    pub program_name: Option<String>,
    pub runtime_backend: Option<EbpfRuntimeBackend>,
    pub sampling_per_sec: Option<u32>,
    pub stream_seconds: Option<u32>,
    pub enable_kernel_stream: Option<bool>,
    pub debug_breakpoints: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfCompilerDiagnostic {
    pub line: usize,
    pub column: usize,
    pub end_column: usize,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfCheckResponse {
    pub ok: bool,
    pub message: String,
    pub diagnostics: Vec<EbpfCompilerDiagnostic>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EbpfRemoteCheckSubmitRequest {
    pub code: String,
    pub agent_id: String,
    pub program_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EbpfRemoteCheckStatusQuery {
    pub job_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EbpfRemoteCheckCancelRequest {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfRemoteCheckResponse {
    pub job_id: String,
    pub state: RunnerJobState,
    pub agent_id: Option<String>,
    pub message: String,
    pub result: Option<EbpfCheckResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfCheckBackend {
    pub agent_id: String,
    pub isolation: RunnerAgentIsolation,
    pub state: RunnerAgentState,
    pub available_slots: u16,
    pub max_concurrent: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfCheckBackendInventory {
    pub local_available: bool,
    pub agents: Vec<EbpfCheckBackend>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EbpfCompletionRequest {
    pub code: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfCompletionItem {
    pub label: String,
    pub insert_text: String,
    pub detail: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfCompletionResponse {
    pub ok: bool,
    pub items: Vec<EbpfCompletionItem>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfRunResponse {
    pub success: bool,
    pub stage: String,
    pub message: String,
    pub compile_stdout: String,
    pub compile_stderr: String,
    pub load_stdout: String,
    pub load_stderr: String,
    pub pin_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<EbpfDebugInfo>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EbpfDebugRejectedBreakpoint {
    pub line: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EbpfDebugInfo {
    pub mode: String,
    pub session_id: Option<String>,
    pub requested_lines: Vec<u32>,
    pub instrumented_lines: Vec<u32>,
    pub rejected: Vec<EbpfDebugRejectedBreakpoint>,
}

impl EbpfRunResponse {
    pub fn validation_error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            stage: "validation".to_string(),
            message: message.into(),
            compile_stdout: String::new(),
            compile_stderr: String::new(),
            load_stdout: String::new(),
            load_stderr: String::new(),
            pin_path: None,
            debug: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub capability: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub code: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EbpfDetachRequest {
    pub pin_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfDetachResponse {
    pub ok: bool,
    pub message: String,
    pub detached: Vec<String>,
    pub clean: bool,
    pub safety_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfAttachmentListResponse {
    pub pin_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfAttachmentDetail {
    pub pin_path: String,
    pub source: String,
    pub program_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfAttachmentDetailListResponse {
    pub attachments: Vec<EbpfAttachmentDetail>,
}
