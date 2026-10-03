use super::{
    contract::{self, SnapshotResponse},
    errors::HttpError,
    security, TaskContentHttpState,
};
use crate::models::collaboration::TaskId;
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

enum Endpoint {
    Create,
    Read(TaskId),
    Replace(TaskId),
    Transition(TaskId),
}

fn endpoint(request: &Request) -> Result<Endpoint, HttpError> {
    if request.uri().query().is_some() {
        return Err(HttpError::invalid_request());
    }
    let path = request.uri().path();
    let method = request.method();
    if path == "/platform/v1/tasks" {
        return if *method == Method::POST {
            Ok(Endpoint::Create)
        } else {
            Err(HttpError::method_not_allowed("POST"))
        };
    }
    let missing = || HttpError::new(StatusCode::NOT_FOUND, "not_found");
    let tail = path
        .strip_prefix("/platform/v1/tasks/")
        .ok_or_else(missing)?;
    if tail.is_empty() {
        return Err(missing());
    }
    let parts: Vec<_> = tail.splitn(3, '/').collect();
    if parts.len() > 2 || (parts.len() == 2 && !matches!(parts[1], "content" | "status")) {
        return Err(missing());
    }
    // Parse the raw URI segment, not a decoded/normalized path alias for an identifier.
    let id = parts[0].parse().map_err(|_| HttpError::invalid_request())?;
    match (parts.get(1).copied(), method) {
        (None, &Method::GET) => Ok(Endpoint::Read(id)),
        (Some("content"), &Method::PUT) => Ok(Endpoint::Replace(id)),
        (Some("status"), &Method::POST) => Ok(Endpoint::Transition(id)),
        (None, _) => Err(HttpError::method_not_allowed("GET")),
        (Some("content"), _) => Err(HttpError::method_not_allowed("PUT")),
        (Some("status"), _) => Err(HttpError::method_not_allowed("POST")),
        _ => Err(missing()),
    }
}

pub(super) async fn dispatch(
    State(state): State<TaskContentHttpState>,
    request: Request,
) -> Response {
    match handle(state, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn handle(state: TaskContentHttpState, request: Request) -> Result<Response, HttpError> {
    let endpoint = endpoint(&request)?;
    let token = security::credentials(request.headers(), request.method(), &state.origin)?;
    let bytes = security::request_bytes(request, matches!(endpoint, Endpoint::Read(_))).await?;
    let (status, response) = match endpoint {
        Endpoint::Create => {
            let request: contract::Create = contract::parse(&bytes)?;
            let _ = request.schema_version;
            let snapshot = state
                .source
                .create_session_content_task(
                    &token,
                    &state.workspace,
                    request.task_id,
                    request.manifest,
                )
                .await?;
            (StatusCode::CREATED, SnapshotResponse::from(snapshot))
        }
        Endpoint::Read(id) => {
            let content = state
                .source
                .get_session_content_task(&token, &state.workspace, id)
                .await?
                .ok_or_else(|| HttpError::dispatched(StatusCode::NOT_FOUND, "not_found"))?;
            (StatusCode::OK, SnapshotResponse::try_from(content)?)
        }
        Endpoint::Replace(id) => {
            let request: contract::Replace = contract::parse(&bytes)?;
            let _ = request.schema_version;
            let snapshot = state
                .source
                .replace_session_content_task(
                    &token,
                    &state.workspace,
                    id,
                    request.expected_revision,
                    request.manifest,
                )
                .await?;
            (StatusCode::OK, SnapshotResponse::from(snapshot))
        }
        Endpoint::Transition(id) => {
            let request: contract::Transition = contract::parse(&bytes)?;
            let _ = request.schema_version;
            let status = request.status()?;
            let snapshot = state
                .source
                .transition_session_content_task(
                    &token,
                    &state.workspace,
                    id,
                    request.expected_revision,
                    status,
                )
                .await?;
            (StatusCode::OK, SnapshotResponse::from(snapshot))
        }
    };
    // JSON contains only committed service results. Text stays inert data, never HTML or a path.
    Ok((status, Json(response)).into_response())
}
