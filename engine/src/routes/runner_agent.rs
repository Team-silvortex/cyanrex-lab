use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    models::runner_agent::{
        RunnerAgentHeartbeatRequest, RunnerAgentRegisterRequest, RunnerAgentRegistrationResponse,
    },
    services::{
        runner_agent_authenticator::RunnerAgentSignedRequest,
        runner_agent_registry::{RunnerAgentAccessError, RunnerAgentRegistryError},
    },
    AppState,
};

const AGENT_ID_HEADER: &str = "x-cyanrex-agent-id";
const TIMESTAMP_HEADER: &str = "x-cyanrex-agent-timestamp";
const NONCE_HEADER: &str = "x-cyanrex-agent-nonce";
const SIGNATURE_HEADER: &str = "x-cyanrex-agent-signature";

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
        Ok(agent) => {
            let agent_ids = state.runner_agent_registry.active_agent_ids();
            match state
                .runner_agent_authenticator
                .issue(&agent.agent_id, &agent_ids)
            {
                Ok(credential) => {
                    let mut response = Json(RunnerAgentRegistrationResponse {
                        agent,
                        credential,
                        signature_scheme: state.runner_agent_authenticator.signature_scheme(),
                    })
                    .into_response();
                    response
                        .headers_mut()
                        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
                    response
                }
                Err(_) => error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "runner agent credential capacity reached",
                ),
            }
        }
        Err(error) => registry_error(error),
    }
}

pub async fn heartbeat(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let agent_id = match authenticate_signed_request(
        state.as_ref(),
        &headers,
        "POST",
        "/runner/agent/heartbeat",
        &body,
    ) {
        Ok(agent_id) => agent_id,
        Err(response) => return response,
    };
    let request = match serde_json::from_slice::<RunnerAgentHeartbeatRequest>(&body) {
        Ok(request) => request,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "invalid heartbeat payload"),
    };
    if request.agent_id != agent_id {
        return error_response(StatusCode::UNAUTHORIZED, "runner agent identity mismatch");
    }
    match state.runner_agent_registry.heartbeat(request) {
        Ok(agent) => Json(agent).into_response(),
        Err(error) => registry_error(error),
    }
}

pub(crate) fn authenticate_signed_request<'a>(
    state: &AppState,
    headers: &'a HeaderMap,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<&'a str, Response> {
    let field = |name: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty())
    };
    let Some(agent_id) = field(AGENT_ID_HEADER) else {
        return Err(signature_error());
    };
    let Some(timestamp) = field(TIMESTAMP_HEADER) else {
        return Err(signature_error());
    };
    let Some(nonce) = field(NONCE_HEADER) else {
        return Err(signature_error());
    };
    let Some(signature) = field(SIGNATURE_HEADER) else {
        return Err(signature_error());
    };
    state
        .runner_agent_authenticator
        .verify(RunnerAgentSignedRequest {
            agent_id,
            method,
            path,
            timestamp,
            nonce,
            signature,
            body,
        })
        .map_err(|_| signature_error())?;
    Ok(agent_id)
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

fn signature_error() -> Response {
    error_response(StatusCode::UNAUTHORIZED, "invalid runner agent signature")
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
