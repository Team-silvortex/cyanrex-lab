use crate::{
    platform_http::errors::HttpError,
    services::{
        auth_service::durable_source::{
            DurableAuthError, SessionCommandError, SessionFileDownloadError,
        },
        collaboration_identity_store::IdentityStoreError,
    },
};
use axum::http::StatusCode;

/// Dispatched errors never expose store/namespace diagnostics or infer delivery or rollback.
pub(super) fn source(error: SessionFileDownloadError) -> HttpError {
    use SessionFileDownloadError as D;
    use StatusCode as S;
    let (status, code) = match error {
        D::PermissionDenied => (S::NOT_FOUND, "not_found"),
        D::Session(SessionCommandError::InvalidSession)
        | D::Session(SessionCommandError::Authentication(DurableAuthError::InvalidCredentials)) => {
            (S::UNAUTHORIZED, "invalid_session")
        }
        D::Session(SessionCommandError::Identity(
            IdentityStoreError::AccessDenied
            | IdentityStoreError::ScopeInactive
            | IdentityStoreError::IdentityInactive
            | IdentityStoreError::IdentityRetired
            | IdentityStoreError::StaleIdentity,
        )) => (S::FORBIDDEN, "access_denied"),
        _ => (S::SERVICE_UNAVAILABLE, "service_unavailable"),
    };
    HttpError::dispatched(status, code)
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
