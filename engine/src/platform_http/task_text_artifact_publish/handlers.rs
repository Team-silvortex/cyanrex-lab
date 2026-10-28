use super::{contract::Create, errors::publication_error, TaskTextArtifactPublishHttpState};
use crate::{
    models::collaboration::TaskId,
    platform_http::{
        admission, contract, errors::HttpError, security, text_artifact::contract as text,
    },
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
        .and_then(|tail| tail.strip_suffix("/text-artifacts"))
        .filter(|tail| !tail.is_empty() && !tail.contains('/'))
        .ok_or_else(|| HttpError::new(StatusCode::NOT_FOUND, "not_found"))?;
    // Raw canonical identifier only; percent-decoding must not introduce path aliases.
    let id = tail.parse().map_err(|_| HttpError::invalid_request())?;
    if *request.method() != Method::POST {
        return Err(HttpError::method_not_allowed("POST"));
    }
    Ok(id)
}

pub(super) async fn dispatch(
    State(state): State<TaskTextArtifactPublishHttpState>,
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
    state: TaskTextArtifactPublishHttpState,
    id: TaskId,
    token: String,
    request: Request,
) -> Result<Response, HttpError> {
    let bytes = security::bounded_request_bytes(request, false, text::MAX_WIRE_BYTES).await?;
    let request: Create = contract::parse(&bytes)?;
    let command = request.into_command(id, state.scope)?;
    // No HTTP deadline wraps the dispatched command. It alone owns Task/current authority,
    // original storage, new bytes, deferred checks and commit; an error proves no rollback.
    let committed = state
        .backend
        .create(
            &token,
            command.expected,
            command.artifact,
            command.revision,
            command.text,
        )
        .await
        .map_err(publication_error)?;
    Ok((
        StatusCode::CREATED,
        Json(text::TextResponse::from(committed)),
    )
        .into_response())
}
