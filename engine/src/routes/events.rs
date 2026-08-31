use std::sync::Arc;

use crate::services::event_bus::EventQueryFilters;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use crate::{models::event::Event, AppState};

const DEFAULT_EVENT_LIMIT: usize = 200;
const MAX_EVENT_LIMIT: usize = 500;

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub category: Option<String>,
    pub severity: Option<String>,
    pub limit: Option<usize>,
    pub since_minutes: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EventsExportQuery {
    pub format: Option<String>,
    pub category: Option<String>,
    pub severity: Option<String>,
    pub since_minutes: Option<i64>,
    pub start: Option<String>,
    pub end: Option<String>,
}

pub async fn list_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<EventsQuery>,
) -> Json<Vec<Event>> {
    let username = current_username_from_headers(&state, &headers).await;
    let category_filter = sanitize_category(&query.category);
    let severity_filter = sanitize_severity(&query.severity);
    let limit = resolve_event_limit(query.limit);

    let filters = EventQueryFilters {
        category: category_filter.as_deref(),
        severity: severity_filter.as_deref(),
        limit: Some(limit),
        since_minutes: query.since_minutes,
        start: parse_rfc3339(query.start.as_deref()),
        end: parse_rfc3339(query.end.as_deref()),
    };
    let events = state
        .event_bus
        .snapshot_for_user_filtered(&username, filters)
        .await;

    Json(events)
}

pub async fn export_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<EventsExportQuery>,
) -> Response {
    let username = current_username_from_headers(&state, &headers).await;
    let category_filter = sanitize_category(&query.category);
    let severity_filter = sanitize_severity(&query.severity);
    let filters = EventQueryFilters {
        category: category_filter.as_deref(),
        severity: severity_filter.as_deref(),
        limit: None,
        since_minutes: query.since_minutes,
        start: parse_rfc3339(query.start.as_deref()),
        end: parse_rfc3339(query.end.as_deref()),
    };
    let events = state
        .event_bus
        .snapshot_for_user_filtered(&username, filters)
        .await;
    let format = query
        .format
        .as_deref()
        .unwrap_or("json")
        .to_ascii_lowercase();

    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let filename = format!("cyanrex-events-{timestamp}.{format}");

    if format == "csv" {
        let body = to_csv(&events);
        return build_download_response("text/csv; charset=utf-8", &filename, body);
    }

    match serde_json::to_string(&events) {
        Ok(body) => build_download_response("application/json; charset=utf-8", &filename, body),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "ok": false, "message": format!("failed to serialize events: {error}") })),
        )
            .into_response(),
    }
}

pub async fn delete_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<EventsExportQuery>,
) -> Json<serde_json::Value> {
    let username = current_username_from_headers(&state, &headers).await;
    let category_filter = sanitize_category(&query.category);
    let severity_filter = sanitize_severity(&query.severity);
    let deleted_count = state
        .event_bus
        .delete_user_events_filtered(
            &username,
            category_filter.as_deref(),
            severity_filter.as_deref(),
            query.since_minutes,
            parse_rfc3339(query.start.as_deref()),
            parse_rfc3339(query.end.as_deref()),
        )
        .await;

    Json(serde_json::json!({
        "ok": true,
        "deleted": deleted_count,
    }))
}

pub async fn ws_events(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    let username = current_username_from_headers(&state, &headers).await;
    ws.on_upgrade(move |socket| handle_ws(socket, state, username))
}

pub async fn unread_count(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let username = current_username_from_headers(&state, &headers).await;
    let unread = state.event_bus.unread_count_for_user(&username).await;
    Json(serde_json::json!({ "unread": unread }))
}

pub async fn mark_read(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let username = current_username_from_headers(&state, &headers).await;
    state.event_bus.mark_all_read_for_user(&username).await;
    Json(serde_json::json!({ "ok": true }))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>, username: String) {
    let mut receiver = state.event_bus.subscribe();

    loop {
        tokio::select! {
            maybe_msg = socket.recv() => {
                match maybe_msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            maybe_event = receiver.recv() => {
                match maybe_event {
                    Ok(event) => {
                        if event.username != username {
                            continue;
                        }
                        let text = match serde_json::to_string(&event) {
                            Ok(value) => value,
                            Err(_) => continue,
                        };
                        if socket.send(Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
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

fn sanitize_category(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|raw| raw.trim().to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "kernel" | "platform"))
}

fn sanitize_severity(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|raw| raw.trim().to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "success" | "warning" | "error"))
}

fn resolve_event_limit(raw: Option<usize>) -> usize {
    raw.filter(|value| *value > 0)
        .map(|value| value.min(MAX_EVENT_LIMIT))
        .unwrap_or(DEFAULT_EVENT_LIMIT)
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
    response
}
