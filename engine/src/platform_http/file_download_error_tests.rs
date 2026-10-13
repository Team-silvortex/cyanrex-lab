//! Explicit private wire expectations shared by both file HTTP error adapters.
use super::HttpError;
use crate::services::{
    auth_service::durable_source::{
        DurableAuthError as A, SessionCommandError as S, SessionFileDownloadError as D,
    },
    collaboration_identity_store::IdentityStoreError as I,
    file_recipient_grant_store::FileRecipientGrantStoreError as R,
    file_upload_publication::FileUploadPublicationStoreError as P,
};
use axum::{
    body::to_bytes,
    http::{header, StatusCode},
    response::IntoResponse,
};
use std::time::Duration;

pub(in crate::platform_http) async fn assert_error(
    error: HttpError,
    status: StatusCode,
    code: &str,
) {
    let response = error.into_response();
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    for forbidden in [header::ALLOW, header::LOCATION, header::SET_COOKIE] {
        assert!(!response.headers().contains_key(forbidden));
    }
    let bytes = tokio::time::timeout(Duration::from_secs(1), to_bytes(response.into_body(), 1024))
        .await
        .expect("bounded static error response")
        .expect("static error response bytes");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("static error JSON");
    // Exact equality rejects internal diagnostics, namespace/store details and extra fields.
    assert_eq!(
        value,
        serde_json::json!({
            "schema_version": 1,
            "error": {"code": code, "outcome": "unconfirmed"}
        })
    );
}

pub(in crate::platform_http) async fn assert_projection_matrix(project: fn(D) -> HttpError) {
    assert_error(
        project(D::PermissionDenied),
        StatusCode::NOT_FOUND,
        "not_found",
    )
    .await;
    for error in [S::InvalidSession, S::Authentication(A::InvalidCredentials)] {
        assert_error(
            project(D::Session(error)),
            StatusCode::UNAUTHORIZED,
            "invalid_session",
        )
        .await;
    }
    for error in [
        I::AccessDenied,
        I::ScopeInactive,
        I::IdentityInactive,
        I::IdentityRetired,
        I::StaleIdentity,
    ] {
        assert_error(
            project(D::Session(S::Identity(error))),
            StatusCode::FORBIDDEN,
            "access_denied",
        )
        .await;
    }
    // These are explicitly 503, including otherwise tempting 400/409/429 classifications.
    for error in [
        R::StorageUnavailable,
        R::UnsupportedSchema,
        R::NamespaceNotEmpty,
        R::ScopeMismatch,
        R::IncarnationMismatch,
        R::InvalidInput,
        R::InvalidRecord,
        R::Conflict,
        R::QuotaExceeded,
    ] {
        assert_error(
            project(D::Recipient(error)),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
        .await;
    }
    for error in [
        P::StorageUnavailable,
        P::UnsupportedSchema,
        P::NamespaceNotEmpty,
        P::ScopeMismatch,
        P::IncarnationMismatch,
        P::InvalidInput,
        P::InvalidRecord,
        P::Conflict,
        P::CapacityExhausted,
    ] {
        assert_error(
            project(D::Publication(error)),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
        .await;
    }
    assert_error(
        project(D::InvalidNamespace),
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
    )
    .await;
    for error in [
        A::StorageUnavailable,
        A::UnsupportedSchema,
        A::SourceMismatch,
        A::LegacyDataPresent,
        A::InvalidRecord,
        A::InvalidInput,
        A::AccountExists,
        A::RateLimited,
    ] {
        assert_error(
            project(D::Session(S::Authentication(error))),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
        .await;
    }
    for error in [S::AccountMismatch, S::SelfDeletionUnsupported] {
        assert_error(
            project(D::Session(error)),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
        .await;
    }
    for error in [
        I::StorageUnavailable,
        I::UnsupportedSchema,
        I::ScopeNotFound,
        I::ScopeConflict,
        I::IdentityConflict,
        I::StaleAccess,
        I::AuditNotEnabled,
        I::AuditContextRequired,
        I::IdentityAuditNotEnabled,
        I::IdentityAuditContextRequired,
        I::AuditBootstrapRequired,
        I::CommandConflict,
        I::LastAuthorityManager,
        I::InvalidRecord,
    ] {
        assert_error(
            project(D::Session(S::Identity(error))),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
        .await;
    }
}
