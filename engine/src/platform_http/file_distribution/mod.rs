//! Trusted unmounted composition only, never a listener or an isolation of the privileged Engine.
use super::{
    errors::HttpError,
    file_chunk_read::{self, FileChunkReadHttpState},
    file_download_manifest::{self, FileDownloadManifestHttpState},
    security, TaskContentHttpConfigError,
};
use crate::services::auth_service::durable_source::{
    DurableAuthSource, SessionFileChunkReadWorkspace,
};
use axum::{
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    Router,
};

/// One supplied Source and one frozen chunk workspace; manifest configuration is derived privately.
/// Clones share each independent HTTP budget and the original workspace's source-byte budget.
#[derive(Clone)]
pub struct FileDistributionHttpState {
    manifests: FileDownloadManifestHttpState,
    chunks: FileChunkReadHttpState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FileDistributionHttpConfigError {
    #[error("file distribution requires one canonical HTTPS or loopback HTTP origin")]
    InvalidOrigin,
    #[error("each file distribution HTTP capacity must be between 1 and 16")]
    InvalidCapacity,
    #[error("file distribution requires a matching cached trusted workspace")]
    InvalidWorkspace,
}

impl From<TaskContentHttpConfigError> for FileDistributionHttpConfigError {
    fn from(error: TaskContentHttpConfigError) -> Self {
        match error {
            TaskContentHttpConfigError::InvalidOrigin => Self::InvalidOrigin,
            TaskContentHttpConfigError::InvalidCapacity => Self::InvalidCapacity,
        }
    }
}

impl FileDistributionHttpState {
    pub fn new(
        source: DurableAuthSource,
        workspace: SessionFileChunkReadWorkspace,
        origin: &str,
    ) -> Result<Self, FileDistributionHttpConfigError> {
        Self::with_capacities(source, workspace, origin, 4, 4)
    }

    /// Explicit independent control/binary budgets; this is not a global memory or worker quota.
    pub fn with_capacities(
        source: DurableAuthSource,
        workspace: SessionFileChunkReadWorkspace,
        origin: &str,
        manifest_capacity: usize,
        chunk_capacity: usize,
    ) -> Result<Self, FileDistributionHttpConfigError> {
        security::validate_origin(origin)?;
        if !(1..=16).contains(&manifest_capacity) || !(1..=16).contains(&chunk_capacity) {
            return Err(FileDistributionHttpConfigError::InvalidCapacity);
        }
        let manifest_workspace = workspace
            .manifest_workspace_for(&source)
            .map_err(|_| FileDistributionHttpConfigError::InvalidWorkspace)?;
        let manifests = FileDownloadManifestHttpState::with_capacity(
            source.clone(),
            manifest_workspace,
            origin,
            manifest_capacity,
        )?;
        let chunks =
            FileChunkReadHttpState::with_capacity(source, workspace, origin, chunk_capacity)?;
        Ok(Self { manifests, chunks })
    }
}

/// Explicit router construction only, with no app mounting, Session issuer or installed-store check.
pub fn build_file_distribution_router(state: FileDistributionHttpState) -> Router {
    // Merge only our private fallback-free routes, never caller routers or either old full router.
    Router::new()
        .merge(file_download_manifest::routes().with_state(state.manifests))
        .merge(file_chunk_read::routes().with_state(state.chunks))
        .fallback(not_found)
        .layer(middleware::from_fn(security::response_headers))
}

async fn not_found() -> Response {
    // Every other path stays private without credential/body collection or resource dispatch.
    HttpError::new(StatusCode::NOT_FOUND, "not_found").into_response()
}
