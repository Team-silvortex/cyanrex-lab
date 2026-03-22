use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct EbpfRunRequest {
    pub code: String,
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
        }
    }
}
