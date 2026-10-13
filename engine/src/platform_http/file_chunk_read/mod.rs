//! Standalone unmounted current-recipient binary transport, never a durable delivery receipt.
mod body;
mod contract;
mod errors;
mod handlers;

use super::{security, TaskContentHttpConfigError};
use crate::services::auth_service::durable_source::{
    DurableAuthSource, SessionFileChunkReadWorkspace,
};
use axum::{middleware, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Trusted host composition only. Clients cannot supply scopes, storage roots or owners.
#[derive(Clone)]
pub struct FileChunkReadHttpState {
    source: DurableAuthSource,
    workspace: SessionFileChunkReadWorkspace,
    origin: String,
    admission: Arc<Semaphore>,
}

impl FileChunkReadHttpState {
    pub fn new(
        source: DurableAuthSource,
        workspace: SessionFileChunkReadWorkspace,
        origin: &str,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(source, workspace, origin, 4)
    }

    /// Clones share HTTP admission; independent states and source workspaces remain additive.
    pub fn with_capacity(
        source: DurableAuthSource,
        workspace: SessionFileChunkReadWorkspace,
        origin: &str,
        capacity: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        security::validate_origin(origin)?;
        if !(1..=16).contains(&capacity) {
            return Err(TaskContentHttpConfigError::InvalidCapacity);
        }
        Ok(Self {
            source,
            workspace,
            origin: origin.to_owned(),
            admission: Arc::new(Semaphore::new(capacity)),
        })
    }
}

/// Explicit router construction only; no application merge, listener or Session issuance.
pub fn build_file_chunk_read_router(state: FileChunkReadHttpState) -> Router {
    routes()
        .fallback(handlers::dispatch)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}

/// Only the private exact route; callers cannot inject a router or replace its authoritative state.
pub(super) fn routes() -> Router<FileChunkReadHttpState> {
    Router::new().route("/platform/v1/file-chunks", any(handlers::dispatch))
}
