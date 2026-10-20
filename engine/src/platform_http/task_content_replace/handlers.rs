use super::TaskContentReplaceHttpState;
use crate::{
    models::collaboration::TaskId,
    platform_http::{admission, contract, errors::HttpError, security},
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
        .and_then(|tail| tail.strip_suffix("/content"))
        .filter(|tail| !tail.is_empty() && !tail.contains('/'))
        .ok_or_else(|| HttpError::new(StatusCode::NOT_FOUND, "not_found"))?;
    // Parse the original URI segment only; no decoded or normalized identifier alias.
    let id = tail.parse().map_err(|_| HttpError::invalid_request())?;
    if *request.method() != Method::PUT {
        return Err(HttpError::method_not_allowed("PUT"));
    }
    Ok(id)
}

pub(super) async fn dispatch(
    State(state): State<TaskContentReplaceHttpState>,
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
    // Capacity refusal after cheap route/header checks never polls body or storage.
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
    state: TaskContentReplaceHttpState,
    id: TaskId,
    token: String,
    request: Request,
) -> Result<Response, HttpError> {
    let bytes = security::request_bytes(request, false).await?;
    // Preserve field occurrences through strict top-level and nested model deserializers.
    let request: contract::Replace = contract::parse(&bytes)?;
    let _ = request.schema_version;
    // The body deadline ends before dispatch. One retained command owns old/new byte checks,
    // fresh authorization and confirmed commit; no HTTP retry, compensation or rollback claim.
    let committed = state
        .backend
        .replace(&token, id, request.expected_revision, request.manifest)
        .await?;
    Ok((
        StatusCode::OK,
        Json(contract::SnapshotResponse::from(committed)),
    )
        .into_response())
}
