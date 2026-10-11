use super::super::{
    admission,
    contract::{self as shared_contract, SnapshotResponse},
    errors::HttpError,
    handlers::{endpoint_at, Endpoint},
    security,
};
use super::{contract, CatalogTaskContentHttpState};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

pub(super) async fn dispatch(
    State(state): State<CatalogTaskContentHttpState>,
    request: Request,
) -> Response {
    let endpoint = match endpoint_at(&request, "/platform/v1/catalog-tasks") {
        Ok(endpoint) => endpoint,
        Err(error) => return error.into_response(),
    };
    let token = match security::credentials(request.headers(), request.method(), &state.origin) {
        Ok(token) => token,
        Err(error) => return error.into_response(),
    };
    // Reuse the same fail-fast budget and retain each admitted slot through response-body end/drop.
    let permit = match admission::acquire(&state.admission) {
        Ok(permit) => permit,
        Err(error) => return error.into_response(),
    };
    let response = match handle(state, endpoint, token, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    admission::response(response, permit)
}

async fn handle(
    state: CatalogTaskContentHttpState,
    endpoint: Endpoint,
    token: String,
    request: Request,
) -> Result<Response, HttpError> {
    let bytes = security::request_bytes(request, matches!(endpoint, Endpoint::Read(_))).await?;
    let (status, response) = match endpoint {
        Endpoint::Create => {
            let request: contract::Create = shared_contract::parse(&bytes)?;
            let _ = request.schema_version;
            let reference = request.definition_ref.0.into_reference();
            let snapshot = state
                .source
                .create_session_catalog_content_task(
                    &token,
                    &state.workspace,
                    request.task_id,
                    &reference,
                    request.manifest,
                )
                .await?;
            (StatusCode::CREATED, SnapshotResponse::from(snapshot))
        }
        Endpoint::Read(id) => {
            let content = state
                .source
                .get_session_catalog_content_task(&token, &state.workspace, id)
                .await?
                .ok_or_else(|| HttpError::dispatched(StatusCode::NOT_FOUND, "not_found"))?;
            (StatusCode::OK, SnapshotResponse::try_from(content)?)
        }
        Endpoint::Replace(id) => {
            let request: shared_contract::Replace = shared_contract::parse(&bytes)?;
            let _ = request.schema_version;
            let snapshot = state
                .source
                .replace_session_catalog_content_task(
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
            let request: shared_contract::Transition = shared_contract::parse(&bytes)?;
            let _ = request.schema_version;
            let status = request.status()?;
            let snapshot = state
                .source
                .transition_session_catalog_content_task(
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
    // Committed service results only: this projection neither executes providers nor renders text.
    Ok((status, Json(response)).into_response())
}
