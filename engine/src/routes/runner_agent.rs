use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    models::runner_agent::{RunnerAgentHeartbeatRequest, RunnerAgentRegisterRequest},
    services::runner_agent_registry::{RunnerAgentAccessError, RunnerAgentRegistryError},
    AppState,
};

pub async fn register(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<RunnerAgentRegisterRequest>,
) -> Response {
    if let Err(error) = state
        .runner_agent_registry
        .authorize(bearer_token(&headers))
    {
        return access_error(error);
    }
    match state.runner_agent_registry.register(request) {
        Ok(agent) => Json(agent).into_response(),
        Err(error) => registry_error(error),
    }
}

pub async fn heartbeat(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<RunnerAgentHeartbeatRequest>,
) -> Response {
    if let Err(error) = state
        .runner_agent_registry
        .authorize(bearer_token(&headers))
    {
        return access_error(error);
    }
    match state.runner_agent_registry.heartbeat(request) {
        Ok(agent) => Json(agent).into_response(),
        Err(error) => registry_error(error),
    }
}

pub async fn inventory(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.runner_agent_registry.inventory())
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
}

fn access_error(error: RunnerAgentAccessError) -> Response {
    let (status, message) = match error {
        RunnerAgentAccessError::Disabled => (
            StatusCode::SERVICE_UNAVAILABLE,
            "runner agent registration is disabled",
        ),
        RunnerAgentAccessError::Unauthorized => {
            (StatusCode::UNAUTHORIZED, "invalid runner agent token")
        }
    };
    error_response(status, message)
}

fn registry_error(error: RunnerAgentRegistryError) -> Response {
    match error {
        RunnerAgentRegistryError::Invalid(message) => {
            error_response(StatusCode::BAD_REQUEST, &message)
        }
        RunnerAgentRegistryError::NotFound => {
            error_response(StatusCode::NOT_FOUND, "runner agent is not registered")
        }
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"ok": false, "message": message})),
    )
        .into_response()
}
