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
