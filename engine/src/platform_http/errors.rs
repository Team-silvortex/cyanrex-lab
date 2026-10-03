use crate::services::{
    artifact_store::ArtifactStoreError,
    auth_service::durable_source::{DurableAuthError, SessionCommandError, SessionTaskError},
    collaboration_identity_store::IdentityStoreError,
    task_store::TaskStoreError,
};
use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

pub(super) struct HttpError {
    status: StatusCode,
    code: &'static str,
    outcome: &'static str,
    allow: Option<&'static str>,
}

impl HttpError {
    pub(super) fn new(status: StatusCode, code: &'static str) -> Self {
        Self {
            status,
            code,
            outcome: "not_attempted",
            allow: None,
        }
    }

    pub(super) fn invalid_request() -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request")
    }

    pub(super) fn method_not_allowed(allow: &'static str) -> Self {
        let mut error = Self::new(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed");
        error.allow = Some(allow);
        error
    }

    pub(super) fn dispatched(status: StatusCode, code: &'static str) -> Self {
        Self {
            status,
            code,
            outcome: "unconfirmed",
            allow: None,
        }
    }
}

impl From<SessionTaskError> for HttpError {
    fn from(value: SessionTaskError) -> Self {
        use StatusCode as S;
        // The HTTP boundary cannot infer rollback from any service error, including a final
        // authorization check or lost commit acknowledgement. Never expose storage diagnostics.
        let (status, code) = match value {
            SessionTaskError::Session(SessionCommandError::InvalidSession)
            | SessionTaskError::Session(SessionCommandError::Authentication(
                DurableAuthError::InvalidCredentials,
            )) => (S::UNAUTHORIZED, "invalid_session"),
            SessionTaskError::Session(SessionCommandError::Identity(
                IdentityStoreError::AccessDenied
                | IdentityStoreError::ScopeInactive
                | IdentityStoreError::IdentityInactive
                | IdentityStoreError::IdentityRetired
                | IdentityStoreError::StaleIdentity,
            )) => (S::FORBIDDEN, "access_denied"),
            SessionTaskError::Task(TaskStoreError::NotFound)
            | SessionTaskError::Artifact(ArtifactStoreError::NotFound) => {
                (S::NOT_FOUND, "not_found")
            }
            SessionTaskError::Task(TaskStoreError::Conflict) => (S::CONFLICT, "conflict"),
            SessionTaskError::Task(TaskStoreError::StaleRevision) => {
                (S::CONFLICT, "stale_revision")
            }
            SessionTaskError::Task(TaskStoreError::InvalidTransition) => {
                (S::CONFLICT, "invalid_transition")
            }
            SessionTaskError::Task(
                TaskStoreError::InvalidInput | TaskStoreError::ScopeMismatch,
            )
            | SessionTaskError::Artifact(
                ArtifactStoreError::InvalidInput | ArtifactStoreError::ScopeMismatch,
            ) => (S::BAD_REQUEST, "invalid_request"),
            _ => (S::SERVICE_UNAVAILABLE, "service_unavailable"),
        };
        Self::dispatched(status, code)
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    schema_version: u8,
    error: ErrorDetail,
}
#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    outcome: &'static str,
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorResponse {
                schema_version: 1,
                error: ErrorDetail {
                    code: self.code,
                    outcome: self.outcome,
                },
            }),
        )
            .into_response();
        if let Some(allow) = self.allow {
            response
                .headers_mut()
                .insert(header::ALLOW, HeaderValue::from_static(allow));
        }
        response
    }
}
