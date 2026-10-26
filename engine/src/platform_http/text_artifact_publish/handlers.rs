use super::TextArtifactPublishHttpState;
use crate::platform_http::{
    admission, contract, errors::HttpError, security, text_artifact::contract as text_contract,
};
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

fn endpoint(request: &Request) -> Result<(), HttpError> {
    if request.uri().path() != "/platform/v1/text-artifacts" {
        return Err(HttpError::new(StatusCode::NOT_FOUND, "not_found"));
    }
    if request.uri().query().is_some() {
        return Err(HttpError::invalid_request());
    }
    if *request.method() != Method::POST {
        return Err(HttpError::method_not_allowed("POST"));
    }
    Ok(())
}

pub(super) async fn dispatch(
    State(state): State<TextArtifactPublishHttpState>,
    request: Request,
) -> Response {
    if let Err(error) = endpoint(&request) {
        return error.into_response();
    }
    let token = match security::credentials(request.headers(), request.method(), &state.origin) {
        Ok(token) => token,
        Err(error) => return error.into_response(),
    };
    let permit = match admission::acquire(&state.admission) {
        Ok(permit) => permit,
        Err(error) => return error.into_response(),
    };
    let response = match handle(state, token, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    admission::response(response, permit)
}

async fn handle(
    state: TextArtifactPublishHttpState,
    token: String,
    request: Request,
) -> Result<Response, HttpError> {
    let bytes =
        security::bounded_request_bytes(request, false, text_contract::MAX_WIRE_BYTES).await?;
    let request: text_contract::Create = contract::parse(&bytes)?;
    let _ = request.schema_version;
    let (artifact, revision) = (request.artifact_id, request.revision_id);
    let (title, text) = request.into_text_parts()?;
    // Only body collection has an HTTP deadline. The retained command owns authorization,
    // original-storage checks, deferred work and commit; errors cannot authorize a retry.
    let committed = state
        .backend
        .create(&token, artifact, revision, title, text)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(text_contract::TextResponse::from(committed)),
    )
        .into_response())
}
