use serde::{Deserialize, Serialize};

use super::module::ModuleInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRequest {
    pub command_type: CommandType,
    pub module_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandType {
    ListModules,
    StartModule,
    StopModule,
    RunExperiment,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResponse {
    pub ok: bool,
    pub command_type: CommandType,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modules: Option<Vec<ModuleInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<ModuleInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_path: Option<String>,
}
