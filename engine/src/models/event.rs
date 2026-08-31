use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub username: String,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub event_type: String,
    pub category: EventCategory,
    pub severity: EventSeverity,
    pub color: EventColor,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    Kernel,
    Platform,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventSeverity {
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventColor {
    Green,
    Yellow,
    Red,
}

impl EventSeverity {
    pub fn color(self) -> EventColor {
        match self {
            Self::Success => EventColor::Green,
            Self::Warning => EventColor::Yellow,
            Self::Error => EventColor::Red,
        }
    }
}
