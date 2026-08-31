use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventOverflowPolicyDto {
    DropOldest,
    DropNew,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventSettingsResponse {
    pub max_records: usize,
    pub overflow_policy: EventOverflowPolicyDto,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateEventSettingsRequest {
    pub max_records: usize,
    pub overflow_policy: EventOverflowPolicyDto,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateEventSettingsResponse {
    pub ok: bool,
    pub message: String,
    pub settings: Option<EventSettingsResponse>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCompilerSettingsRequest {
    pub resident: bool,
}

#[derive(Debug, Serialize)]
pub struct CompilerSettingsResponse {
    pub resident: bool,
    pub strategy: &'static str,
}

#[derive(Debug, Serialize)]
pub struct UpdateCompilerSettingsResponse {
    pub ok: bool,
    pub message: String,
    pub settings: CompilerSettingsResponse,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CompilerOperationMetricsResponse {
    pub total_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub errors: u64,
    pub rejected: u64,
    pub in_flight: u64,
    pub in_flight_peak: u64,
    pub avg_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceMetricsResponse {
    pub check: CompilerOperationMetricsResponse,
    pub completion: CompilerOperationMetricsResponse,
}
