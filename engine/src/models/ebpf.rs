use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EbpfRuntimeBackend {
    Bpftool,
    Aya,
}

impl Default for EbpfRuntimeBackend {
    fn default() -> Self {
        Self::Bpftool
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EbpfRunRequest {
    pub code: String,
    pub template_id: Option<String>,
    pub program_name: Option<String>,
    pub runtime_backend: Option<EbpfRuntimeBackend>,
    pub sampling_per_sec: Option<u32>,
    pub stream_seconds: Option<u32>,
    pub enable_kernel_stream: Option<bool>,
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
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EbpfTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub capability: String,
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
