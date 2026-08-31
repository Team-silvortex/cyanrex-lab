use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserScript {
    pub id: String,
    pub username: String,
    pub title: String,
    pub script: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveScriptRequest {
    pub title: String,
    pub script: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteScriptRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaveScriptResponse {
    pub ok: bool,
    pub message: String,
    pub record: Option<UserScript>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeleteScriptResponse {
    pub ok: bool,
    pub message: String,
}
