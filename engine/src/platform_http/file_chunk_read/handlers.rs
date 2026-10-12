use super::{body, contract, errors, FileChunkReadHttpState};
use crate::{
    models::collaboration::WorkspaceRef,
    platform_http::{admission, errors::HttpError, security},
    services::auth_service::durable_source::SessionFileChunkReadRequest,
};
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
};

fn endpoint(
    request: &Request,
    scope: WorkspaceRef,
) -> Result<SessionFileChunkReadRequest, HttpError> {
    if request.uri().path() != "/platform/v1/file-chunks" {
        return Err(HttpError::new(StatusCode::NOT_FOUND, "not_found"));
    }
    if *request.method() != Method::GET {
        return Err(HttpError::method_not_allowed("GET"));
    }
    // This is neither a range nor a conditional resource. Never silently ignore such selectors.
    if request
        .headers()
        .keys()
        .any(|name| name.as_str() == "range" || name.as_str().starts_with("if-"))
    {
        return Err(HttpError::invalid_request());
    }
    contract::chunk_request(request.uri().query(), scope)
}

pub(super) async fn dispatch(
    State(state): State<FileChunkReadHttpState>,
    request: Request,
) -> Response {
    let selected = match endpoint(&request, state.workspace.file_scope()) {
        Ok(selected) => selected,
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
    if let Err(error) = security::request_bytes(request, true).await {
        return admission::response(error.into_response(), permit);
    }
    // Exactly one existing current-Session command owns authorization and private verified IO.
    let committed = state
        .source
        .read_session_file_chunk(&token, &state.workspace, &selected)
        .await;
    // Start the first-frame budget immediately on return, before any metadata projection.
    let deadline = body::first_frame_deadline();
    let chunk = match committed {
        Ok(chunk) => chunk,
        Err(error) => return admission::response(errors::source(error).into_response(), permit),
    };
    let headers = match contract::response_headers(&chunk) {
        Ok(headers) => headers,
        Err(error) => return admission::response(error.into_response(), permit),
    };
    let mut response = Response::new(body::response(chunk, permit, deadline));
    *response.headers_mut() = headers;
    // Success deliberately bypasses the JSON-body gate: both leases belong to the data owner,
    // including Bytes clones/slices retained after the response body has reached EOF or dropped.
    response
}
