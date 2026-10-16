//! Separate, unmounted text publication transport; no installer, issuer or Task/journal writer.
use super::{config, security, TaskContentHttpConfigError};
use crate::services::auth_service::durable_source::{DurableAuthSource, SessionArtifactWorkspace};
use axum::{middleware, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

mod contract;
mod handlers;

#[derive(Clone)]
pub struct TextArtifactHttpState {
    source: DurableAuthSource,
    workspace: SessionArtifactWorkspace,
    origin: String,
    admission: Arc<Semaphore>,
}

impl TextArtifactHttpState {
    pub fn new(
        source: DurableAuthSource,
        workspace: SessionArtifactWorkspace,
        origin: &str,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(source, workspace, origin, 4)
    }

    /// Clones share this budget; new states and Task-content states have independent budgets.
    pub fn with_capacity(
        source: DurableAuthSource,
        workspace: SessionArtifactWorkspace,
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

/// Explicit construction only. No implicit merge with Task-content or the running Engine router.
pub fn build_text_artifact_router(state: TextArtifactHttpState) -> Router {
    Router::new()
        .route("/platform/v1/text-artifacts", any(handlers::dispatch))
        .route(
            "/platform/v1/text-artifacts/{artifact_id}/revisions/{revision_id}/{sha256}",
            any(handlers::dispatch),
        )
        .fallback(handlers::dispatch)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}
