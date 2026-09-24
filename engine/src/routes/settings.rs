use std::sync::Arc;

use axum::{
    extract::{rejection::JsonRejection, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    models::settings::{
        CompilerSettingsResponse, EventOverflowPolicyDto, EventSettingsResponse,
        PerformanceMetricsResponse, UpdateCompilerSettingsRequest, UpdateCompilerSettingsResponse,
        UpdateEventSettingsRequest, UpdateEventSettingsResponse,
    },
    services::event_bus::EventOverflowPolicy,
    AppState,
};

pub async fn get_compiler_settings(
    State(state): State<Arc<AppState>>,
) -> Json<CompilerSettingsResponse> {
    Json(compiler_settings(&state))
}

pub async fn get_performance_metrics(
    State(state): State<Arc<AppState>>,
) -> Json<PerformanceMetricsResponse> {
    Json(state.performance_snapshot())
}

pub async fn update_compiler_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateCompilerSettingsRequest>,
) -> Json<UpdateCompilerSettingsResponse> {
    state
        .ebpf_loader
        .set_resident_compiler(payload.resident)
        .await;
    Json(UpdateCompilerSettingsResponse {
        ok: true,
        message: "compiler mode updated".to_string(),
        settings: compiler_settings(&state),
    })
}

fn compiler_settings(state: &AppState) -> CompilerSettingsResponse {
    let resident = state.ebpf_loader.resident_compiler_enabled();
    CompilerSettingsResponse {
        resident,
        strategy: if resident {
            "resident_cache"
        } else {
            "on_demand"
        },
    }
}

pub async fn get_event_settings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    let username =
        match crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await {
            Some(session) => session.username,
            None => {
                return private_event_settings_response((
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"ok": false, "message": "missing auth session"})),
                ));
            }
        };

    match state.event_bus.read_settings_for_user(&username).await {
        Ok(settings) => private_event_settings_response(Json(EventSettingsResponse {
            max_records: settings.max_records,
            overflow_policy: to_dto_policy(settings.overflow_policy),
        })),
        Err(_) => private_event_settings_response((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"ok": false, "message": "event settings are unavailable"})),
        )),
    }
}

pub async fn update_event_settings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    payload: Result<Json<UpdateEventSettingsRequest>, JsonRejection>,
) -> Response {
    let username = match crate::routes::auth::current_session_from_headers(state.as_ref(), &headers)
        .await
    {
        Some(session) => session.username,
        None => {
            return event_settings_update_error(StatusCode::UNAUTHORIZED, "missing auth session");
        }
    };

    let payload = match payload {
        Ok(Json(payload)) => payload,
        Err(rejection) => {
            return event_settings_update_error(
                rejection.status(),
                "invalid event settings request",
            );
        }
    };
    match state
        .event_bus
        .update_settings_confirmed(
            &username,
            payload.max_records,
            from_dto_policy(payload.overflow_policy),
        )
        .await
    {
        Ok(settings) => private_event_settings_response((
            StatusCode::OK,
            Json(UpdateEventSettingsResponse {
                ok: true,
                message: "event settings updated".to_string(),
                settings: Some(EventSettingsResponse {
                    max_records: settings.max_records,
                    overflow_policy: to_dto_policy(settings.overflow_policy),
                }),
            }),
        )),
        Err(_) => event_settings_update_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "event settings update could not be confirmed; verify state before retrying",
        ),
    }
}

fn event_settings_update_error(status: StatusCode, message: &str) -> Response {
    private_event_settings_response((
        status,
        Json(UpdateEventSettingsResponse {
            ok: false,
            message: message.to_owned(),
            settings: None,
        }),
    ))
}

fn private_event_settings_response(value: impl IntoResponse) -> Response {
    let mut response = value.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
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
