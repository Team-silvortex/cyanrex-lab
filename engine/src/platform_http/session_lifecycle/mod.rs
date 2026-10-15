//! Explicit Session lifecycle adapter, never a login issuer or live application composition.

mod handlers;

use super::{security, TaskContentHttpConfigError};
use crate::services::auth_service::durable_source::DurableAuthSource;
use axum::{middleware, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Trusted host configuration only; construction neither connects nor installs a source.
#[derive(Clone)]
pub struct SessionLifecycleHttpState {
    source: DurableAuthSource,
    origin: String,
    admission: Arc<Semaphore>,
}

impl SessionLifecycleHttpState {
    pub fn new(
        source: DurableAuthSource,
        origin: &str,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(source, origin, 4)
    }

    /// Clones share response-lifetime slots; separately constructed states are independent.
    pub fn with_capacity(
        source: DurableAuthSource,
        origin: &str,
        max_in_flight: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        security::validate_origin(origin)?;
        if !(1..=16).contains(&max_in_flight) {
            return Err(TaskContentHttpConfigError::InvalidCapacity);
        }
        Ok(Self {
            source,
            origin: origin.to_owned(),
            admission: Arc::new(Semaphore::new(max_in_flight)),
        })
    }
}

/// Explicit, unmounted router only; no listener, login, enrollment or teaching-auth fallback.
pub fn build_session_lifecycle_router(state: SessionLifecycleHttpState) -> Router {
    Router::new()
        .route("/platform/v1/session", any(handlers::dispatch))
        .route("/platform/v1/session/logout", any(handlers::dispatch))
        .fallback(handlers::dispatch)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}
