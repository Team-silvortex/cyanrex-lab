use chrono::{DateTime, Utc};
use serde::Serialize;

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
    pub checks: Vec<EnvironmentCheckItem>,
}
