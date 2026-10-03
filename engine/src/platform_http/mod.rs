//! Explicit, unmounted HTTP boundary for private schema-3 Task content.
//!
//! Trusted composition supplies an already initialized durable source, workspace and browser
//! origin. This module never installs storage, listens, issues cookies, provides CORS, or adopts
//! the live teaching application's AuthService. The integration owner must separately arrange
//! secure same-origin transport and delivery of the dedicated durable-session cookie.

mod contract;
mod errors;
mod handlers;
mod security;

use crate::services::auth_service::durable_source::{
    DurableAuthSource, SessionTaskContentWorkspace,
};
use axum::{middleware, routing::any, Router};

/// Trusted configuration only: no request deserializer, environment reads or storage operations.
#[derive(Clone)]
pub struct TaskContentHttpState {
    source: DurableAuthSource,
    workspace: SessionTaskContentWorkspace,
    origin: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TaskContentHttpConfigError {
    #[error("platform HTTP requires one canonical HTTPS or loopback HTTP origin")]
    InvalidOrigin,
}

impl TaskContentHttpState {
    pub fn new(
        source: DurableAuthSource,
        workspace: SessionTaskContentWorkspace,
        origin: &str,
    ) -> Result<Self, TaskContentHttpConfigError> {
        security::validate_origin(origin)?;
        Ok(Self {
            source,
            workspace,
            origin: origin.to_owned(),
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
