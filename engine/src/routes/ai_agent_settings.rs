use crate::{
    models::ai_agent::{UpdateAiAgentSettingsRequest, UpdateAiAgentSettingsResponse},
    services::ai_agent_settings::AiAgentSettingsError,
    AppState,
};
use axum::{
    extract::{rejection::JsonRejection, Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

pub async fn no_store(request: Request, next: Next) -> Response {
    let private_route = request.uri().path() == "/settings/ai-agents";
    let response = next.run(request).await;
    if private_route {
        private(response)
    } else {
        response
    }
}
pub async fn get_settings(State(state): State<Arc<AppState>>) -> Response {
    match state.ai_agent_settings.get().await {
        Ok(settings) => private(Json(settings)),
        Err(error) => failure(error),
    }
}

pub async fn update_settings(
    State(state): State<Arc<AppState>>,
    payload: Result<Json<UpdateAiAgentSettingsRequest>, JsonRejection>,
) -> Response {
    let request = match payload {
        Ok(Json(request)) => request,
        Err(rejection) => {
            let status = match rejection.status() {
                StatusCode::PAYLOAD_TOO_LARGE => StatusCode::PAYLOAD_TOO_LARGE,
                StatusCode::UNSUPPORTED_MEDIA_TYPE => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                _ => StatusCode::BAD_REQUEST,
            };
            return error(status, "invalid AI Agent settings request");
        }
    };
    match state.ai_agent_settings.update(request).await {
        Ok(settings) => private(Json(UpdateAiAgentSettingsResponse { ok: true, settings })),
        Err(error) => failure(error),
    }
}

fn failure(failure: AiAgentSettingsError) -> Response {
    let status = match failure {
        AiAgentSettingsError::InvalidInput => StatusCode::BAD_REQUEST,
        AiAgentSettingsError::Conflict => StatusCode::CONFLICT,
        AiAgentSettingsError::StorageUnavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    error(status, &failure.to_string())
}

fn error(status: StatusCode, message: &str) -> Response {
    private((
        status,
        Json(serde_json::json!({"ok": false, "message": message})),
    ))
}

fn private(response: impl IntoResponse) -> Response {
    let mut response = response.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
