use chrono::{DateTime, Utc};

use crate::models::event::{Event, EventCategory, EventSeverity};

pub(crate) fn normalize_category_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| raw.to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "kernel" | "platform"))
}

pub(crate) fn normalize_severity_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| raw.to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "success" | "warning" | "error"))
}

pub(crate) fn matches_event_filters(
    event: &Event,
    category: Option<&str>,
    severity: Option<&str>,
    since_cutoff: Option<DateTime<Utc>>,
    start: Option<&DateTime<Utc>>,
    end: Option<&DateTime<Utc>>,
) -> bool {
    if let Some(category_filter) = category {
        let expected = match category_filter {
            "kernel" => EventCategory::Kernel,
            "platform" => EventCategory::Platform,
            _ => return false,
        };
        if event.category != expected {
            return false;
        }
    }

    if let Some(severity_filter) = severity {
        let expected = match severity_filter {
            "success" => EventSeverity::Success,
            "warning" => EventSeverity::Warning,
            "error" => EventSeverity::Error,
            _ => return false,
        };
        if event.severity != expected {
            return false;
        }
    }

    if let Some(cutoff) = since_cutoff {
        if event.timestamp < cutoff {
            return false;
        }
    }

    if let Some(start_time) = start {
        if event.timestamp < *start_time {
            return false;
        }
    }

    if let Some(end_time) = end {
        if event.timestamp > *end_time {
            return false;
        }
    }

    true
}

pub(crate) fn filter_events(
    mut events: Vec<Event>,
    category: Option<&str>,
    severity: Option<&str>,
    limit: Option<usize>,
    since_minutes: Option<i64>,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
) -> Vec<Event> {
    let category_filter = normalize_category_filter(category);
    let severity_filter = normalize_severity_filter(severity);
    let since_cutoff = since_minutes.filter(|value| *value > 0).map(|value| {
        // UTC now is intentionally evaluated at filter time so in-memory fallback
        // follows the same semantics as DB-side filter.
        Utc::now() - chrono::Duration::minutes(value)
    });

    events.retain(|event| {
        matches_event_filters(
            event,
            category_filter.as_deref(),
            severity_filter.as_deref(),
            since_cutoff,
            start.as_ref(),
            end.as_ref(),
        )
    });

    if let Some(max) = limit {
        if events.len() > max {
            events = events.split_off(events.len() - max);
        }
    }

    events
}
