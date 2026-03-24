use std::sync::Arc;

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::{
    models::settings::{
        EventOverflowPolicyDto, EventSettingsResponse, UpdateEventSettingsRequest,
        UpdateEventSettingsResponse,
    },
    services::event_bus::EventOverflowPolicy,
    AppState,
};

pub async fn get_event_settings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> (StatusCode, Json<EventSettingsResponse>) {
    let username =
        match crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await {
            Some(session) => session.username,
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(EventSettingsResponse {
                        max_records: 500,
                        overflow_policy: EventOverflowPolicyDto::DropOldest,
                    }),
                );
            }
        };

    let settings = state.event_bus.settings_for_user(&username).await;
    (
        StatusCode::OK,
        Json(EventSettingsResponse {
            max_records: settings.max_records,
            overflow_policy: to_dto_policy(settings.overflow_policy),
        }),
    )
}

pub async fn update_event_settings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<UpdateEventSettingsRequest>,
) -> (StatusCode, Json<UpdateEventSettingsResponse>) {
    let username =
        match crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await {
            Some(session) => session.username,
            None => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(UpdateEventSettingsResponse {
                        ok: false,
                        message: "missing auth session".to_string(),
                        settings: None,
                    }),
                );
            }
        };

    match state
        .event_bus
        .update_settings_for_user(
            &username,
            payload.max_records,
            from_dto_policy(payload.overflow_policy),
        )
        .await
    {
        Ok(settings) => (
            StatusCode::OK,
            Json(UpdateEventSettingsResponse {
                ok: true,
                message: "event settings updated".to_string(),
                settings: Some(EventSettingsResponse {
                    max_records: settings.max_records,
                    overflow_policy: to_dto_policy(settings.overflow_policy),
                }),
            }),
        ),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(UpdateEventSettingsResponse {
                ok: false,
                message: error,
                settings: None,
            }),
        ),
    }
}

fn to_dto_policy(policy: EventOverflowPolicy) -> EventOverflowPolicyDto {
    match policy {
        EventOverflowPolicy::DropOldest => EventOverflowPolicyDto::DropOldest,
        EventOverflowPolicy::DropNew => EventOverflowPolicyDto::DropNew,
    }
}

fn from_dto_policy(policy: EventOverflowPolicyDto) -> EventOverflowPolicy {
    match policy {
        EventOverflowPolicyDto::DropOldest => EventOverflowPolicy::DropOldest,
        EventOverflowPolicyDto::DropNew => EventOverflowPolicy::DropNew,
    }
}
