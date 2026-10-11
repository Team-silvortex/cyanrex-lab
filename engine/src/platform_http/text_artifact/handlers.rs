use super::{
    contract::{self, TextResponse},
    TextArtifactHttpState,
};
use crate::{
    models::collaboration::{ArtifactId, ArtifactRef, ArtifactRevisionId, Sha256Digest},
    platform_http::{admission, errors::HttpError, security},
};
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

enum Endpoint {
    Create,
    Read(ArtifactId, ArtifactRevisionId, Sha256Digest),
}
fn endpoint(request: &Request) -> Result<Endpoint, HttpError> {
    if request.uri().query().is_some() {
        return Err(HttpError::invalid_request());
    }
    let path = request.uri().path();
    let method = request.method();
    if path == "/platform/v1/text-artifacts" {
        return if *method == Method::POST {
            Ok(Endpoint::Create)
        } else {
            Err(HttpError::method_not_allowed("POST"))
        };
    }
    let missing = || HttpError::new(StatusCode::NOT_FOUND, "not_found");
    let tail = path
        .strip_prefix("/platform/v1/text-artifacts/")
        .ok_or_else(missing)?;
    let parts: Vec<_> = tail.splitn(5, '/').collect();
    if parts.len() != 4 || parts[1] != "revisions" {
        return Err(missing());
    }
    let artifact = parts[0].parse().map_err(|_| HttpError::invalid_request())?;
    let revision = parts[2].parse().map_err(|_| HttpError::invalid_request())?;
    let digest = parts[3].parse().map_err(|_| HttpError::invalid_request())?;
    if *method == Method::GET {
        Ok(Endpoint::Read(artifact, revision, digest))
    } else {
        Err(HttpError::method_not_allowed("GET"))
    }
}

pub(super) async fn dispatch(
    State(state): State<TextArtifactHttpState>,
    request: Request,
) -> Response {
    let endpoint = match endpoint(&request) {
        Ok(endpoint) => endpoint,
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
    let response = match handle(state, endpoint, token, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    admission::response(response, permit)
}

async fn handle(
    state: TextArtifactHttpState,
    endpoint: Endpoint,
    token: String,
    request: Request,
) -> Result<Response, HttpError> {
    let bytes = security::bounded_request_bytes(
        request,
        matches!(endpoint, Endpoint::Read(..)),
        contract::MAX_WIRE_BYTES,
    )
    .await?;
    let (status, response) = match endpoint {
        Endpoint::Create => {
            let request: contract::Create = crate::platform_http::contract::parse(&bytes)?;
            let _ = request.schema_version;
            let (artifact, revision) = (request.artifact_id, request.revision_id);
            let draft = request.into_draft()?;
            let committed = state
                .source
                .create_session_artifact(&token, &state.workspace, artifact, revision, draft)
                .await?;
            (StatusCode::CREATED, TextResponse::from(committed))
        }
        Endpoint::Read(artifact_id, revision_id, sha256) => {
            let reference = ArtifactRef {
                workspace: state.workspace.scope(),
                artifact_id,
                revision_id,
                sha256,
            };
            let content = state
                .source
                .read_session_artifact(&token, &state.workspace, &reference)
                .await?
                .ok_or_else(|| HttpError::dispatched(StatusCode::NOT_FOUND, "not_found"))?;
            (StatusCode::OK, TextResponse::try_from(content)?)
        }
    };
    Ok((status, Json(response)).into_response())
}
