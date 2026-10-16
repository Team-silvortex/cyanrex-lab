//! Explicit, unmounted owner-private metadata index over current-Session commands.

mod contract;
mod handlers;

use super::{config, security, TaskContentHttpConfigError};
use crate::services::auth_service::durable_source::{
    DurableAuthSource, SessionTaskContentIndexWorkspace,
};
use axum::{middleware, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Trusted composition only: no owner, namespace or catalogue selection from requests.
#[derive(Clone)]
pub struct TaskContentIndexHttpState {
    source: DurableAuthSource,
    workspace: SessionTaskContentIndexWorkspace,
    origin: String,
    admission: Arc<Semaphore>,
}

impl TaskContentIndexHttpState {
    pub fn new(
        source: DurableAuthSource,
        workspace: SessionTaskContentIndexWorkspace,
        origin: &str,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(source, workspace, origin, 4)
    }

    /// Cloned states share slots through response-body completion/drop; separate states do not.
    pub fn with_capacity(
        source: DurableAuthSource,
        workspace: SessionTaskContentIndexWorkspace,
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

/// Standalone read-only router, never merged into the live application automatically.
pub fn build_task_content_index_router(state: TaskContentIndexHttpState) -> Router {
    Router::new()
        .route("/platform/v1/task-index", any(handlers::dispatch))
        .fallback(handlers::dispatch)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}
