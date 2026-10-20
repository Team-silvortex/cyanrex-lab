use super::TaskCreateHttpState;
use crate::{
    models::collaboration::{CoreSchemaVersion, TaskContentManifest, TaskId},
    platform_http::{admission, contract, errors::HttpError, security},
};
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateEmpty {
    schema_version: CoreSchemaVersion,
    task_id: TaskId,
    title: String,
}

fn endpoint(request: &Request) -> Result<(), HttpError> {
    if request.uri().path() != "/platform/v1/tasks" {
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
    State(state): State<TaskCreateHttpState>,
    request: Request,
) -> Response {
    if let Err(error) = endpoint(&request) {
        return error.into_response();
    }
    let token = match security::credentials(request.headers(), request.method(), &state.origin) {
        Ok(token) => token,
        Err(error) => return error.into_response(),
    };
    // Route and authority headers precede admission; capacity refusal never polls body or storage.
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
    state: TaskCreateHttpState,
    token: String,
    request: Request,
) -> Result<Response, HttpError> {
    let bytes = security::request_bytes(request, false).await?;
    let request: CreateEmpty = contract::parse(&bytes)?;
    let _ = request.schema_version;
    // Validate title with the existing model before dispatch; no caller may supply payload or owner.
    TaskContentManifest::new(&request.title, vec![]).map_err(|_| HttpError::invalid_request())?;
    // Body timeout has ended. The connector owns final authorization and confirmed commit;
    // dispatch has no HTTP timeout, retry, compensation or inference of rollback.
    let committed = state
        .backend
        .create(&token, request.task_id, request.title)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(contract::SnapshotResponse::from(committed)),
    )
        .into_response())
}
