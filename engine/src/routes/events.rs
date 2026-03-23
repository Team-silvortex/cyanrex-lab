use std::sync::Arc;

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
    let events = state.event_bus.snapshot_for_user(&username).await;
    Json(apply_filters(
        events,
        &query.category,
        &query.severity,
        query.limit,
        query.since_minutes,
        &query.start,
        &query.end,
    ))
}

pub async fn export_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<EventsExportQuery>,
) -> Response {
    let username = current_username_from_headers(&state, &headers).await;
    let events = state.event_bus.snapshot_for_user(&username).await;
    let filtered = apply_filters(
        events,
        &query.category,
        &query.severity,
        None,
        query.since_minutes,
        &query.start,
        &query.end,
    );
    let format = query
        .format
        .as_deref()
        .unwrap_or("json")
        .to_ascii_lowercase();

    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let filename = format!("cyanrex-events-{timestamp}.{format}");

    if format == "csv" {
        let body = to_csv(&filtered);
        return build_download_response("text/csv; charset=utf-8", &filename, body);
    }

    match serde_json::to_string_pretty(&filtered) {
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
    let events = state.event_bus.snapshot_for_user(&username).await;
    let filtered = apply_filters(
        events.clone(),
        &query.category,
        &query.severity,
        None,
        query.since_minutes,
        &query.start,
        &query.end,
    );

    let to_delete = build_event_key_set(&filtered);
    let retained = events
        .into_iter()
        .filter(|event| !to_delete.contains(&event_key(event)))
        .collect::<Vec<_>>();

    let deleted_count = filtered.len();
    state
        .event_bus
        .replace_user_events(&username, retained)
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

fn apply_filters(
    mut events: Vec<Event>,
    category: &Option<String>,
    severity: &Option<String>,
    limit: Option<usize>,
    since_minutes: Option<i64>,
    start: &Option<String>,
    end: &Option<String>,
) -> Vec<Event> {
    let since_cutoff = since_minutes
        .filter(|minutes| *minutes > 0)
        .map(|minutes| chrono::Utc::now() - chrono::Duration::minutes(minutes));
    if let Some(cutoff) = since_cutoff {
        events.retain(|event| event.timestamp >= cutoff);
    }

    if let Some(start_time) = parse_rfc3339(start.as_deref()) {
        events.retain(|event| event.timestamp >= start_time);
    }

    if let Some(end_time) = parse_rfc3339(end.as_deref()) {
        events.retain(|event| event.timestamp <= end_time);
    }

    if let Some(category_filter) = category.as_deref().filter(|value| !value.is_empty()) {
        events.retain(|event| match category_filter {
            "kernel" => event.category == crate::models::event::EventCategory::Kernel,
            "platform" => event.category == crate::models::event::EventCategory::Platform,
            _ => true,
        });
    }

    if let Some(severity_filter) = severity.as_deref().filter(|value| !value.is_empty()) {
        events.retain(|event| match severity_filter {
            "success" => event.severity == crate::models::event::EventSeverity::Success,
            "warning" => event.severity == crate::models::event::EventSeverity::Warning,
            "error" => event.severity == crate::models::event::EventSeverity::Error,
            _ => true,
        });
    }

    if let Some(max) = limit {
        if events.len() > max {
            events = events.split_off(events.len() - max);
        }
    }

    events
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

fn event_key(event: &Event) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        event.timestamp.to_rfc3339(),
        event.source,
        event.event_type,
        match event.category {
            crate::models::event::EventCategory::Kernel => "kernel",
            crate::models::event::EventCategory::Platform => "platform",
        },
        serde_json::to_string(&event.payload).unwrap_or_else(|_| "{}".to_string()),
    )
}

fn build_event_key_set(events: &[Event]) -> std::collections::HashSet<String> {
    events.iter().map(event_key).collect()
}
