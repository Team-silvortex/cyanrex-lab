//! Standalone unmounted current-recipient manifest observation, never reusable byte authority.
mod contract;
mod errors;
mod handlers;
mod response;

use super::{config, security, TaskContentHttpConfigError};
use crate::services::auth_service::durable_source::{
    DurableAuthSource, SessionFileDownloadWorkspace,
};
use axum::{middleware, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Trusted host composition only; request selectors cannot choose scope, owner or storage.
#[derive(Clone)]
pub struct FileDownloadManifestHttpState {
    source: DurableAuthSource,
    workspace: SessionFileDownloadWorkspace,
    origin: String,
    admission: Arc<Semaphore>,
}

impl FileDownloadManifestHttpState {
    pub fn new(
        source: DurableAuthSource,
        workspace: SessionFileDownloadWorkspace,
        origin: &str,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(source, workspace, origin, 4)
    }

    /// Clones share slots through response-body completion/drop; independent states are additive.
    pub fn with_capacity(
        source: DurableAuthSource,
        workspace: SessionFileDownloadWorkspace,
        origin: &str,
        capacity: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        config::validate(origin, &[capacity])?;
        Ok(Self {
            source,
            workspace,
            origin: origin.to_owned(),
            admission: Arc::new(Semaphore::new(capacity)),
        })
    }
}

/// Explicit router construction only, without a listener, application merge or Session issuance.
pub fn build_file_download_manifest_router(state: FileDownloadManifestHttpState) -> Router {
    routes()
        .fallback(handlers::dispatch)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}

/// No fallback or middleware here; a private composition owns exactly one of each.
pub(super) fn routes() -> Router<FileDownloadManifestHttpState> {
    Router::new().route("/platform/v1/file-manifests", any(handlers::dispatch))
}
