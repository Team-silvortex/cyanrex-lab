use std::sync::Arc;

use crate::services::event_bus::{EventMutationError, EventQueryFilters, EventReadError};
use axum::{
    extract::{rejection::QueryRejection, ws::WebSocketUpgrade, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::{models::event::Event, AppState};

mod stream;

const DEFAULT_EVENT_LIMIT: usize = 200;
const MAX_EVENT_LIMIT: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventsQuery {
    pub category: Option<String>,
    pub severity: Option<String>,
    pub limit: Option<usize>,
    pub since_minutes: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
    #[serde(rename = "username")]
    pub ignored_username: Option<String>,
    #[serde(rename = "format")]
    pub ignored_format: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventsExportQuery {
    pub format: Option<String>,
    pub category: Option<String>,
    pub severity: Option<String>,
    pub since_minutes: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
    #[serde(rename = "username")]
    pub ignored_username: Option<String>,
    #[serde(rename = "limit")]
    pub ignored_limit: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventsDeleteQuery {
    pub category: Option<String>,
    pub severity: Option<String>,
    pub since_minutes: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
    // Retain these legacy ignored fields; neither can select another owner's data.
    #[serde(rename = "username")]
    pub ignored_username: Option<String>,
    #[serde(rename = "format")]
    pub ignored_format: Option<String>,
}

pub async fn list_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    query: Result<Query<EventsQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return read_error(EventReadError::InvalidFilters);
    };
    let username = current_username_from_headers(&state, &headers).await;
    let limit = resolve_event_limit(query.limit);
    let filters = match read_filters(
        &query.category,
        &query.severity,
        Some(limit),
        query.since_minutes,
        &query.start,
        &query.end,
    ) {
        Ok(filters) => filters,
        Err(error) => return read_error(error),
    };
    match state
        .event_bus
        .read_snapshot_for_user_filtered(&username, filters)
        .await
    {
        Ok(events) => private_response(Json(events)),
        Err(error) => read_error(error),
    }
}

pub async fn export_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    query: Result<Query<EventsExportQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return read_error(EventReadError::InvalidFilters);
    };
    let username = current_username_from_headers(&state, &headers).await;
    let filters = match read_filters(
        &query.category,
        &query.severity,
        None,
        query.since_minutes,
        &query.start,
        &query.end,
    ) {
        Ok(filters) => filters,
        Err(error) => return read_error(error),
    };
    let format = query
        .format
        .as_deref()
        .unwrap_or("json")
        .trim()
        .to_ascii_lowercase();
    if !matches!(format.as_str(), "json" | "csv") {
        return read_error(EventReadError::InvalidFilters);
    }
    let events = match state
        .event_bus
        .read_snapshot_for_user_filtered(&username, filters)
        .await
    {
        Ok(events) => events,
        Err(error) => return read_error(error),
    };

    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let filename = format!("cyanrex-events-{timestamp}.{format}");

    if format == "csv" {
        let body = to_csv(&events);
        return build_download_response("text/csv; charset=utf-8", &filename, body);
    }

    match serde_json::to_string(&events) {
        Ok(body) => build_download_response("application/json; charset=utf-8", &filename, body),
        Err(error) => {
            tracing::warn!(%error, "event export serialization failed");
            private_response((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "ok": false, "message": "event export is unavailable" })),
            ))
        }
    }
}

pub async fn delete_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    query: Result<Query<EventsDeleteQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return mutation_error(EventMutationError::InvalidFilters);
    };
    let username = current_username_from_headers(&state, &headers).await;
    let filters = match read_filters(
        &query.category,
        &query.severity,
        None,
        query.since_minutes,
        &query.start,
        &query.end,
    ) {
        Ok(filters) => filters,
        Err(_) => return mutation_error(EventMutationError::InvalidFilters),
    };
    match state
        .event_bus
        .delete_user_events_confirmed(&username, filters)
        .await
    {
        Ok(deleted) => {
            private_response(Json(serde_json::json!({ "ok": true, "deleted": deleted })))
        }
        Err(error) => mutation_error(error),
    }
}

pub async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    let username = current_username_from_headers(&state, &headers).await;
    // Subscribe before completing the handshake so the client's open notification is a barrier.
    let receiver = state.event_bus.subscribe_for_user(&username);
    ws.on_upgrade(move |socket| stream::handle_ws(socket, receiver, username))
}

pub async fn unread_count(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let username = current_username_from_headers(&state, &headers).await;
    match state.event_bus.read_unread_count_for_user(&username).await {
        Ok(unread) => private_response(Json(serde_json::json!({ "unread": unread }))),
        Err(error) => read_error(error),
    }
}

pub async fn mark_read(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let username = current_username_from_headers(&state, &headers).await;
    match state
        .event_bus
        .acknowledge_all_read_for_user(&username)
        .await
    {
        Ok(()) => private_response(Json(serde_json::json!({ "ok": true }))),
        Err(error) => mutation_error(error),
    }
}

async fn current_username_from_headers(state: &Arc<AppState>, headers: &HeaderMap) -> String {
    crate::routes::auth::current_session_from_headers(state.as_ref(), headers)
        .await
        .map(|session| session.username)
        .unwrap_or_else(|| "unknown".to_string())
}

fn parse_rfc3339(value: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    chrono::DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|datetime| datetime.with_timezone(&chrono::Utc))
}

fn mutation_error(error: EventMutationError) -> Response {
    let (status, message) = match error {
        EventMutationError::InvalidFilters => (
            StatusCode::BAD_REQUEST,
            "invalid event deletion filter or time range",
        ),
        EventMutationError::Unconfirmed => (
            StatusCode::SERVICE_UNAVAILABLE,
            "event mutation could not be confirmed; verify state before retrying",
        ),
    };
    private_response((
        status,
        Json(serde_json::json!({ "ok": false, "message": message })),
    ))
}

fn resolve_event_limit(raw: Option<usize>) -> usize {
    raw.filter(|value| *value > 0)
        .map(|value| value.min(MAX_EVENT_LIMIT))
        .unwrap_or(DEFAULT_EVENT_LIMIT)
}

fn read_filters<'a>(
    category: &'a Option<String>,
    severity: &'a Option<String>,
    limit: Option<usize>,
    since_minutes: Option<i64>,
    start_raw: &Option<String>,
    end_raw: &Option<String>,
) -> Result<EventQueryFilters<'a>, EventReadError> {
    let start = parse_rfc3339(start_raw.as_deref());
    let end = parse_rfc3339(end_raw.as_deref());
    if (start_raw.is_some() && start.is_none()) || (end_raw.is_some() && end.is_none()) {
        return Err(EventReadError::InvalidFilters);
    }
    Ok(EventQueryFilters {
        category: category.as_deref(),
        severity: severity.as_deref(),
        limit,
        since_minutes,
        start,
        end,
    })
}

fn read_error(error: EventReadError) -> Response {
    let (status, message) = match error {
        EventReadError::InvalidFilters => (
            StatusCode::BAD_REQUEST,
            "invalid event query, format or time range",
        ),
        EventReadError::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "event storage is unavailable",
        ),
    };
    private_response((
        status,
        Json(serde_json::json!({ "ok": false, "message": message })),
    ))
}

fn private_response(response: impl IntoResponse) -> Response {
    let mut response = response.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn to_csv(events: &[Event]) -> String {
    let mut output = String::from("timestamp,source,event_type,category,severity,color,payload\n");
    for event in events {
        let payload = serde_json::to_string(&event.payload).unwrap_or_else(|_| "{}".to_string());
        output.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            event.timestamp.to_rfc3339(),
            escape_csv(&event.source),
            escape_csv(&event.event_type),
            escape_csv(match event.category {
                crate::models::event::EventCategory::Kernel => "kernel",
                crate::models::event::EventCategory::Platform => "platform",
            }),
            escape_csv(match event.severity {
                crate::models::event::EventSeverity::Success => "success",
                crate::models::event::EventSeverity::Warning => "warning",
                crate::models::event::EventSeverity::Error => "error",
            }),
            escape_csv(match event.color {
                crate::models::event::EventColor::Green => "green",
                crate::models::event::EventColor::Yellow => "yellow",
                crate::models::event::EventColor::Red => "red",
            }),
            escape_csv(&payload),
        ));
    }
    output
}

fn escape_csv(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn build_download_response(content_type: &str, filename: &str, body: String) -> Response {
    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );
    private_response(response)
}
