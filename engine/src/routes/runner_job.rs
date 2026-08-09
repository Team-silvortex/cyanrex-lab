use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    models::{
        runner_agent::RunnerAgentState,
        runner_job::{
            RunnerCompileCheckSubmitRequest, RunnerJobCancelRequest, RunnerJobClaimRequest,
            RunnerJobClaimResponse, RunnerJobResultRequest, RunnerJobSyncRequest,
            RunnerProbeSubmitRequest,
        },
    },
    services::runner_job_queue::RunnerJobQueueError,
    AppState,
};

pub async fn submit_probe(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RunnerProbeSubmitRequest>,
) -> Response {
    if let Some(agent_id) = request.agent_id.as_deref() {
        if state.runner_agent_registry.agent(agent_id).is_none() {
            return error_response(StatusCode::BAD_REQUEST, "target runner agent is unknown");
        }
    }
    match state.runner_job_queue.submit_probe(
        request.agent_id,
        request.message,
        request.timeout_seconds,
    ) {
        Ok(job) => (StatusCode::CREATED, Json(job)).into_response(),
        Err(error) => queue_error(error),
    }
}

pub async fn submit_compile_check(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RunnerCompileCheckSubmitRequest>,
) -> Response {
    if let Some(agent_id) = request.agent_id.as_deref() {
        let Some(agent) = state.runner_agent_registry.agent(agent_id) else {
            return error_response(StatusCode::BAD_REQUEST, "target runner agent is unknown");
        };
        if !agent
            .capabilities
            .iter()
            .any(|capability| capability == "clang_check")
        {
            return error_response(
                StatusCode::BAD_REQUEST,
                "target runner agent does not advertise clang_check",
            );
        }
    }
    match state.runner_job_queue.submit_compile_check(
        request.agent_id,
        request.source,
        request.program_name,
        request.timeout_seconds,
    ) {
        Ok(job) => (StatusCode::CREATED, Json(job)).into_response(),
        Err(error) => queue_error(error),
    }
}

pub async fn cancel(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RunnerJobCancelRequest>,
) -> Response {
    match state.runner_job_queue.cancel(&request.job_id) {
        Ok(job) => Json(job).into_response(),
        Err(error) => queue_error(error),
    }
}

pub async fn inventory(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.runner_job_queue.inventory())
}

pub async fn claim(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let agent_id =
        match signed_identity(state.as_ref(), &headers, "/runner/agent/jobs/claim", &body) {
            Ok(agent_id) => agent_id,
            Err(response) => return response,
        };
    let request = match serde_json::from_slice::<RunnerJobClaimRequest>(&body) {
        Ok(request) => request,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "invalid job claim payload"),
    };
    if request.agent_id != agent_id {
        return identity_mismatch();
    }
    let Some(agent) = state.runner_agent_registry.agent(agent_id) else {
        return error_response(StatusCode::NOT_FOUND, "runner agent is not registered");
    };
    if agent.state != RunnerAgentState::Healthy || agent.available_slots == 0 {
        return error_response(
            StatusCode::CONFLICT,
            "runner agent is not healthy or has no available capacity",
        );
    }
    match state.runner_job_queue.claim(
        agent_id,
        agent.available_slots as usize,
        &agent.capabilities,
    ) {
        Ok(job) => {
            let mut response = Json(RunnerJobClaimResponse { job }).into_response();
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            response
        }
        Err(error) => queue_error(error),
    }
}

pub async fn sync(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
    let agent_id = match signed_identity(state.as_ref(), &headers, "/runner/agent/jobs/sync", &body)
    {
        Ok(agent_id) => agent_id,
        Err(response) => return response,
    };
    let request = match serde_json::from_slice::<RunnerJobSyncRequest>(&body) {
        Ok(request) => request,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "invalid job sync payload"),
    };
    if request.agent_id != agent_id {
        return identity_mismatch();
    }
    match state.runner_job_queue.sync(agent_id, &request.leases) {
        Ok(response) => Json(response).into_response(),
        Err(error) => queue_error(error),
    }
}

pub async fn result(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let agent_id =
        match signed_identity(state.as_ref(), &headers, "/runner/agent/jobs/result", &body) {
            Ok(agent_id) => agent_id,
            Err(response) => return response,
        };
    let request = match serde_json::from_slice::<RunnerJobResultRequest>(&body) {
        Ok(request) => request,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "invalid job result payload"),
    };
    if request.agent_id != agent_id {
        return identity_mismatch();
    }
    match state.runner_job_queue.complete(request) {
        Ok(job) => Json(job).into_response(),
        Err(error) => queue_error(error),
    }
}

fn signed_identity<'a>(
    state: &AppState,
    headers: &'a HeaderMap,
    path: &str,
    body: &[u8],
) -> Result<&'a str, Response> {
    super::runner_agent::authenticate_signed_request(state, headers, "POST", path, body)
}

fn identity_mismatch() -> Response {
    error_response(StatusCode::UNAUTHORIZED, "runner agent identity mismatch")
}

fn queue_error(error: RunnerJobQueueError) -> Response {
    match error {
        RunnerJobQueueError::Invalid(message) => error_response(StatusCode::BAD_REQUEST, &message),
        RunnerJobQueueError::NotFound => {
            error_response(StatusCode::NOT_FOUND, "runner job was not found")
        }
        RunnerJobQueueError::Conflict(message) => error_response(StatusCode::CONFLICT, &message),
        RunnerJobQueueError::Capacity => {
            error_response(StatusCode::SERVICE_UNAVAILABLE, "runner job queue is full")
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
