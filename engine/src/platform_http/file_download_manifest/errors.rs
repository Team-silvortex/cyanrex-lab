use crate::{
    platform_http::errors::HttpError,
    services::auth_service::durable_source::SessionFileDownloadError,
};

#[cfg(test)]
use crate::services::{
    auth_service::durable_source::{DurableAuthError, SessionCommandError},
    collaboration_identity_store::IdentityStoreError,
};
#[cfg(test)]
use axum::http::StatusCode;

/// Dispatched errors never expose store/namespace diagnostics or infer delivery or rollback.
pub(super) fn source(error: SessionFileDownloadError) -> HttpError {
    error.into()
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;

#[cfg(test)]
#[tokio::test]
async fn manifest_projection_keeps_complete_private_download_error_matrix() {
    crate::platform_http::errors::file_download_tests::assert_projection_matrix(source).await;
}
