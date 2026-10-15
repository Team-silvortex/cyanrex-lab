//! Explicit, unmounted HTTP adapters for Session lifecycle, private Task content,
//! text Artifacts and files.
//!
//! Trusted composition supplies an already initialized durable source, typed workspace and browser
//! origin. This module never installs storage, listens, issues Sessions, provides CORS, or adopts
//! the live teaching application's AuthService. The integration owner must separately arrange
//! secure same-origin transport and delivery of the dedicated durable-session cookie.
//! Only confirmed Session lifecycle logout clears that cookie; other adapters never set it.

mod admission;
mod catalog_task;
mod contract;
mod errors;
#[cfg(unix)]
mod file_chunk_read;
#[cfg(unix)]
mod file_distribution;
#[cfg(unix)]
mod file_download_manifest;
mod handlers;
mod security;
mod session_lifecycle;
mod task_index;
mod text_artifact;
pub use catalog_task::{build_catalog_task_content_router, CatalogTaskContentHttpState};
#[cfg(unix)]
pub use file_chunk_read::{build_file_chunk_read_router, FileChunkReadHttpState};
#[cfg(unix)]
pub use file_distribution::{
    build_file_distribution_router, FileDistributionHttpConfigError, FileDistributionHttpState,
};
#[cfg(unix)]
pub use file_download_manifest::{
    build_file_download_manifest_router, FileDownloadManifestHttpState,
};
pub(crate) use security::validate_origin as validate_platform_origin;
pub use session_lifecycle::{build_session_lifecycle_router, SessionLifecycleHttpState};
pub use task_index::{build_task_content_index_router, TaskContentIndexHttpState};
pub use text_artifact::{build_text_artifact_router, TextArtifactHttpState};

use crate::services::auth_service::durable_source::{
    DurableAuthSource, SessionTaskContentWorkspace,
};
use axum::{middleware, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Trusted configuration only: no request deserializer, environment reads or storage operations.
#[derive(Clone)]
pub struct TaskContentHttpState {
    source: DurableAuthSource,
    workspace: SessionTaskContentWorkspace,
    origin: String,
    admission: Arc<Semaphore>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TaskContentHttpConfigError {
    #[error("platform HTTP requires one canonical HTTPS or loopback HTTP origin")]
    InvalidOrigin,
    #[error("platform HTTP capacity must be between 1 and 16")]
    InvalidCapacity,
}

impl TaskContentHttpState {
    pub fn new(
        source: DurableAuthSource,
        workspace: SessionTaskContentWorkspace,
        origin: &str,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(source, workspace, origin, 4)
    }

    /// Trusted, bounded admission. State clones (including separately built routers) share slots.
    /// A new state gets its own budget; this is not a global process or storage-worker quota.
    pub fn with_capacity(
        source: DurableAuthSource,
        workspace: SessionTaskContentWorkspace,
        origin: &str,
        max_in_flight: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        security::validate_origin(origin)?;
        if !(1..=16).contains(&max_in_flight) {
            return Err(TaskContentHttpConfigError::InvalidCapacity);
        }
        Ok(Self {
            source,
            workspace,
            origin: origin.to_owned(),
            admission: Arc::new(Semaphore::new(max_in_flight)),
        })
    }
}

/// Standalone router only. Never implicitly merged into `application::build_router`.
pub fn build_task_content_router(state: TaskContentHttpState) -> Router {
    Router::new()
        .route("/platform/v1/tasks", any(handlers::dispatch))
        .route("/platform/v1/tasks/{task_id}", any(handlers::dispatch))
        .route(
            "/platform/v1/tasks/{task_id}/content",
            any(handlers::dispatch),
        )
        .route(
            "/platform/v1/tasks/{task_id}/status",
            any(handlers::dispatch),
        )
        .fallback(handlers::dispatch)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}
