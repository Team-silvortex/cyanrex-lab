use crate::{
    platform_http::errors::HttpError,
    services::{
        artifact_store::ArtifactStoreError, auth_service::durable_source::SessionTaskError,
    },
};
use axum::http::StatusCode;

/// Publication conflicts need their own projection; the old Task adapter remains unchanged.
pub(super) fn publication_error(error: SessionTaskError) -> HttpError {
    match error {
        SessionTaskError::Artifact(ArtifactStoreError::Conflict) => {
            HttpError::dispatched(StatusCode::CONFLICT, "conflict")
        }
        SessionTaskError::Artifact(ArtifactStoreError::StaleRevision) => {
            HttpError::dispatched(StatusCode::CONFLICT, "stale_revision")
        }
        error => HttpError::from(error),
    }
}
