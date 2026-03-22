use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRequest {
    pub command_type: CommandType,
    pub module_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandType {
    ListModules,
    StartModule,
    StopModule,
    RunExperiment,
}
