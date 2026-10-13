use super::*;
use crate::services::{
    file_recipient_grant_store::FileRecipientGrantStoreError as R,
    file_upload_publication::FileUploadPublicationStoreError as P,
};
use axum::{body::to_bytes, response::IntoResponse};

async fn assert_error(error: SessionFileDownloadError, status: StatusCode, code: &str) {
    let response = source(error).into_response();
    assert_eq!(response.status(), status);
    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"schema_version":1,"error":{"code":code,"outcome":"unconfirmed"}})
    );
}

#[tokio::test]
async fn dispatched_permission_session_and_identity_errors_have_private_stable_statuses() {
    use SessionFileDownloadError as D;
    assert_error(D::PermissionDenied, StatusCode::NOT_FOUND, "not_found").await;
    for error in [
        SessionCommandError::InvalidSession,
        SessionCommandError::Authentication(DurableAuthError::InvalidCredentials),
    ] {
        assert_error(
            D::Session(error),
            StatusCode::UNAUTHORIZED,
            "invalid_session",
        )
        .await;
    }
    for error in [
        IdentityStoreError::AccessDenied,
        IdentityStoreError::ScopeInactive,
        IdentityStoreError::IdentityInactive,
        IdentityStoreError::IdentityRetired,
        IdentityStoreError::StaleIdentity,
    ] {
        assert_error(
            D::Session(SessionCommandError::Identity(error)),
            StatusCode::FORBIDDEN,
            "access_denied",
        )
        .await;
    }
}

#[tokio::test]
async fn every_remaining_store_configuration_and_auth_failure_is_generic_unconfirmed_503() {
    use SessionFileDownloadError as D;
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
            D::Recipient(error),
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
            D::Publication(error),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
        .await;
    }
    assert_error(
        D::InvalidNamespace,
        StatusCode::SERVICE_UNAVAILABLE,
        "service_unavailable",
    )
    .await;
    for error in [
        DurableAuthError::StorageUnavailable,
        DurableAuthError::UnsupportedSchema,
        DurableAuthError::SourceMismatch,
        DurableAuthError::LegacyDataPresent,
        DurableAuthError::InvalidRecord,
        DurableAuthError::InvalidInput,
        DurableAuthError::AccountExists,
        DurableAuthError::RateLimited,
    ] {
        assert_error(
            D::Session(SessionCommandError::Authentication(error)),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
        .await;
    }
    for error in [
        SessionCommandError::AccountMismatch,
        SessionCommandError::SelfDeletionUnsupported,
    ] {
        assert_error(
            D::Session(error),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
        .await;
    }
    for error in [
        IdentityStoreError::StorageUnavailable,
        IdentityStoreError::UnsupportedSchema,
        IdentityStoreError::ScopeNotFound,
        IdentityStoreError::ScopeConflict,
        IdentityStoreError::IdentityConflict,
        IdentityStoreError::StaleAccess,
        IdentityStoreError::AuditNotEnabled,
        IdentityStoreError::AuditContextRequired,
        IdentityStoreError::IdentityAuditNotEnabled,
        IdentityStoreError::IdentityAuditContextRequired,
        IdentityStoreError::AuditBootstrapRequired,
        IdentityStoreError::CommandConflict,
        IdentityStoreError::LastAuthorityManager,
        IdentityStoreError::InvalidRecord,
    ] {
        assert_error(
            D::Session(SessionCommandError::Identity(error)),
            StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
        )
        .await;
    }
}
