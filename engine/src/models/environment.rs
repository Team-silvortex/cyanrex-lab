use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeMode {
    NativeLinux,
    Wsl2,
    Docker,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentCheckItem {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentReport {
    pub overall_ok: bool,
    pub generated_at: DateTime<Utc>,
    pub runtime_mode: RuntimeMode,
    pub runtime_guidance: String,
    pub checks: Vec<EnvironmentCheckItem>,
}
