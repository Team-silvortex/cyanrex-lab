use super::{contract, errors, response, FileDownloadManifestHttpState};
use crate::{
    models::collaboration::WorkspaceRef,
    platform_http::{admission, errors::HttpError, security},
    services::auth_service::durable_source::SessionFileDownloadRequest,
};
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
};

fn endpoint(
    request: &Request,
    scope: WorkspaceRef,
) -> Result<SessionFileDownloadRequest, HttpError> {
    if request.uri().path() != "/platform/v1/file-manifests" {
        return Err(HttpError::new(StatusCode::NOT_FOUND, "not_found"));
    }
    if *request.method() != Method::GET {
        return Err(HttpError::method_not_allowed("GET"));
    }
    if request
        .headers()
        .keys()
        .any(|name| name.as_str() == "range" || name.as_str().starts_with("if-"))
    {
        return Err(HttpError::invalid_request());
    }
    contract::download_request(request.uri().query(), scope)
}

pub(super) async fn dispatch(
    State(state): State<FileDownloadManifestHttpState>,
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
    let result = handle(state, request, token, selected).await;
    let response = match result {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    // This ordinary JSON body releases its slot at last-frame/end/error/drop. Cloned data Bytes
    // do not retain admission, unlike the distinct verified binary owner/first-frame contract.
    admission::response(response, permit)
}

async fn handle(
    state: FileDownloadManifestHttpState,
    request: Request,
    token: String,
    selected: SessionFileDownloadRequest,
) -> Result<Response, HttpError> {
    security::request_bytes(request, true).await?;
    // Exactly one existing current-recipient metadata command owns records/pins and authorization.
    // No current file, worker, publisher-grant, upload intent or chunk journal is consulted here.
    let manifest = state
        .source
        .get_session_file_download_manifest(&token, &state.workspace, &selected)
        .await
        .map_err(errors::source)?;
    response::manifest(&manifest)
}
