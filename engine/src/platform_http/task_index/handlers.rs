use super::{contract, TaskContentIndexHttpState};
use crate::{
    models::collaboration::TaskRef,
    platform_http::{admission, errors::HttpError, security},
};
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

fn endpoint(request: &Request) -> Result<contract::PageRequest, HttpError> {
    if request.uri().path() != "/platform/v1/task-index" {
        return Err(HttpError::new(StatusCode::NOT_FOUND, "not_found"));
    }
    if *request.method() != Method::GET {
        return Err(HttpError::method_not_allowed("GET"));
    }
    contract::page_request(request.uri().query())
}

pub(super) async fn dispatch(
    State(state): State<TaskContentIndexHttpState>,
    request: Request,
) -> Response {
    let page = match endpoint(&request) {
        Ok(page) => page,
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
    let response = match handle(state, page, token, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    admission::response(response, permit)
}

async fn handle(
    state: TaskContentIndexHttpState,
    page: contract::PageRequest,
    token: String,
    request: Request,
) -> Result<Response, HttpError> {
    security::request_bytes(request, true).await?;
    let after = page.after.map(|task_id| TaskRef {
        workspace: state.backend.scope(),
        task_id,
    });
    // One existing current-Session command owns filtering, validation and final authorization.
    let committed = state.backend.list(&token, after, page.limit).await?;
    Ok((
        StatusCode::OK,
        Json(contract::IndexResponse::from(committed)),
    )
        .into_response())
}
