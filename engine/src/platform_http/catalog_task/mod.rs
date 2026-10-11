//! Explicit, unmounted HTTP composition for frozen catalogue-backed Task content.

mod contract;
mod handlers;

use super::{security, TaskContentHttpConfigError};
use crate::services::auth_service::durable_source::{
    DurableAuthSource, SessionCatalogTaskContentWorkspace,
};
use axum::{middleware, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Trusted configuration only: requests cannot select a catalogue, namespace or owner.
#[derive(Clone)]
pub struct CatalogTaskContentHttpState {
    source: DurableAuthSource,
    workspace: SessionCatalogTaskContentWorkspace,
    origin: String,
    admission: Arc<Semaphore>,
}

impl CatalogTaskContentHttpState {
    pub fn new(
        source: DurableAuthSource,
        workspace: SessionCatalogTaskContentWorkspace,
        origin: &str,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(source, workspace, origin, 4)
    }

    /// Clones share bounded request/response slots; newly constructed states do not.
    pub fn with_capacity(
        source: DurableAuthSource,
        workspace: SessionCatalogTaskContentWorkspace,
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

/// Standalone router only. Never implicitly merged into the live application router.
pub fn build_catalog_task_content_router(state: CatalogTaskContentHttpState) -> Router {
    Router::new()
        .route("/platform/v1/catalog-tasks", any(handlers::dispatch))
        .route(
            "/platform/v1/catalog-tasks/{task_id}",
            any(handlers::dispatch),
        )
        .route(
            "/platform/v1/catalog-tasks/{task_id}/content",
            any(handlers::dispatch),
        )
        .route(
            "/platform/v1/catalog-tasks/{task_id}/status",
            any(handlers::dispatch),
        )
        .fallback(handlers::dispatch)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}
