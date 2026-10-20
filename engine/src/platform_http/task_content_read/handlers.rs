use super::TaskContentReadHttpState;
use crate::{
    models::collaboration::TaskId,
    platform_http::{admission, contract::SnapshotResponse, errors::HttpError, security},
};
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

fn endpoint(request: &Request) -> Result<TaskId, HttpError> {
    if request.uri().query().is_some() {
        return Err(HttpError::invalid_request());
    }
    let tail = request
        .uri()
        .path()
        .strip_prefix("/platform/v1/tasks/")
        .filter(|tail| !tail.is_empty() && !tail.contains('/'))
        .ok_or_else(|| HttpError::new(StatusCode::NOT_FOUND, "not_found"))?;
    // Parse the original URI segment only; never decode or normalize an identifier alias.
    let id = tail.parse().map_err(|_| HttpError::invalid_request())?;
    if *request.method() != Method::GET {
        return Err(HttpError::method_not_allowed("GET"));
    }
    Ok(id)
}

pub(super) async fn dispatch(
    State(state): State<TaskContentReadHttpState>,
    request: Request,
) -> Response {
    let id = match endpoint(&request) {
        Ok(id) => id,
        Err(error) => return error.into_response(),
    };
    let token = match security::credentials(request.headers(), request.method(), &state.origin) {
        Ok(token) => token,
        Err(error) => return error.into_response(),
    };
    let permit = match admission::acquire(&state.admission) {
        Ok(permit) => permit,
        Err(error) => return error.into_response(),
    };
    let response = match handle(state, id, token, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    admission::response(response, permit)
}

async fn handle(
    state: TaskContentReadHttpState,
    id: TaskId,
    token: String,
    request: Request,
) -> Result<Response, HttpError> {
    security::request_bytes(request, true).await?;
    // One command owns exact content and current authority; only a committed value is projected.
    let committed = state
        .backend
        .read(&token, id)
        .await?
        .ok_or_else(|| HttpError::dispatched(StatusCode::NOT_FOUND, "not_found"))?;
    Ok((StatusCode::OK, Json(SnapshotResponse::try_from(committed)?)).into_response())
}
