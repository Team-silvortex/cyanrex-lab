use crate::{
    platform_http::errors::HttpError,
    services::{
        auth_service::durable_source::{
            DurableAuthError, SessionCommandError, SessionFileChunkReadError,
            SessionFileDownloadError,
        },
        collaboration_identity_store::IdentityStoreError,
    },
};
use axum::http::StatusCode;

/// Service dispatch never establishes delivery, rollback or a fresh authorization at body polling.
pub(super) fn source(error: SessionFileChunkReadError) -> HttpError {
    use SessionFileChunkReadError as C;
    use SessionFileDownloadError as D;
    use StatusCode as S;
    let (status, code) = match error {
        C::Metadata(D::PermissionDenied) => (S::NOT_FOUND, "not_found"),
        C::Metadata(D::Session(SessionCommandError::InvalidSession))
        | C::Metadata(D::Session(SessionCommandError::Authentication(
            DurableAuthError::InvalidCredentials,
        ))) => (S::UNAUTHORIZED, "invalid_session"),
        C::Metadata(D::Session(SessionCommandError::Identity(
            IdentityStoreError::AccessDenied
            | IdentityStoreError::ScopeInactive
            | IdentityStoreError::IdentityInactive
            | IdentityStoreError::IdentityRetired
            | IdentityStoreError::StaleIdentity,
        ))) => (S::FORBIDDEN, "access_denied"),
        C::InvalidInput => (S::BAD_REQUEST, "invalid_request"),
        C::CapacityExhausted => (S::SERVICE_UNAVAILABLE, "capacity_exhausted"),
        _ => (S::SERVICE_UNAVAILABLE, "service_unavailable"),
    };
    HttpError::dispatched(status, code)
}
