//! Explicit HTTP adapters for Session login/lifecycle, private Task content,
//! text Artifacts and files.
//! These adapters remain unmounted in the normal teaching application.
//!
//! Trusted composition supplies an already initialized durable source, typed workspace and browser
//! origin. This module never installs storage, listens, provides CORS, or adopts the live teaching
//! application's AuthService. Only the explicit platform host mounts the workspace composition;
//! other adapters remain standalone. The integration owner must arrange secure same-origin
//! transport. Only the explicit login issuer sets the dedicated durable-session cookie after
//! confirmed commit; only confirmed Session lifecycle logout clears it.

mod admission;
mod catalog_task;
mod config;
mod contract;
mod errors;
#[cfg(unix)]
mod file_chunk_read;
#[cfg(unix)]
mod file_distribution;
#[cfg(unix)]
mod file_download_manifest;
#[cfg(unix)]
mod file_selectors;
mod handlers;
mod security;
mod session_lifecycle;
#[cfg(unix)]
mod session_login;
#[cfg(unix)]
mod session_router;
#[cfg(unix)]
mod task_content_read;
#[cfg(unix)]
mod task_content_replace;
#[cfg(unix)]
mod task_create;
mod task_index;
#[cfg(unix)]
mod task_text_artifact_publish;
mod text_artifact;
#[cfg(unix)]
mod text_artifact_publish;
#[cfg(unix)]
mod workspace_router;
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
pub use session_lifecycle::{build_session_lifecycle_router, SessionLifecycleHttpState};
#[cfg(unix)]
pub use session_login::{build_session_login_router, SessionLoginHttpState};
#[cfg(unix)]
pub use session_router::{build_platform_session_router, PlatformSessionHttpState};
#[cfg(unix)]
pub use task_content_read::{build_task_content_read_router, TaskContentReadHttpState};
#[cfg(unix)]
pub use task_content_replace::{build_task_content_replace_router, TaskContentReplaceHttpState};
#[cfg(unix)]
pub use task_create::{build_task_create_router, TaskCreateHttpState};
pub use task_index::{build_task_content_index_router, TaskContentIndexHttpState};
#[cfg(unix)]
pub use task_text_artifact_publish::{
    build_task_text_artifact_publish_router, TaskTextArtifactPublishHttpState,
};
pub use text_artifact::{build_text_artifact_router, TextArtifactHttpState};
#[cfg(unix)]
pub use text_artifact_publish::{build_text_artifact_publish_router, TextArtifactPublishHttpState};
#[cfg(unix)]
pub use workspace_router::{build_platform_workspace_router, PlatformWorkspaceHttpState};

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
        config::validate(origin, &[max_in_flight])?;
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
