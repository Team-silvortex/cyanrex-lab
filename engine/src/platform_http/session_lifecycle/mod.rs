//! Explicit Session lifecycle adapter, never a login issuer or live application composition.

mod handlers;

use super::{config, security, TaskContentHttpConfigError};
#[cfg(unix)]
use crate::platform_host::{PlatformSessionConnector, PlatformStorageConnector};
use crate::services::auth_service::durable_source::{
    DurableAuthError, DurableAuthSource, DurableSession,
};
use axum::{middleware, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Trusted host configuration only; construction neither connects nor installs a source.
#[derive(Clone)]
pub struct SessionLifecycleHttpState {
    backend: Backend,
    origin: String,
    admission: Arc<Semaphore>,
}

#[derive(Clone)]
enum Backend {
    Source(DurableAuthSource),
    #[cfg(unix)]
    Platform(Arc<PlatformSessionConnector>),
}

impl Backend {
    async fn validate_session(
        &self,
        token: &str,
    ) -> Result<Option<DurableSession>, DurableAuthError> {
        match self {
            Self::Source(source) => source.validate_session(token).await,
            #[cfg(unix)]
            Self::Platform(connector) => connector.validate_session(token).await,
        }
    }

    async fn logout(&self, token: &str) -> Result<(), DurableAuthError> {
        match self {
            Self::Source(source) => source.logout(token).await,
            #[cfg(unix)]
            Self::Platform(connector) => connector.logout(token).await,
        }
    }
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
        Self::configured(Backend::Source(source), origin, max_in_flight)
    }

    /// Select retained platform storage only. Origin comes from that original configuration;
    /// no Source/pool fallback, connection, listener or Session issuance is constructed here.
    #[cfg(unix)]
    pub fn with_platform_storage(
        storage: PlatformStorageConnector,
        max_in_flight: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        let connector = Arc::new(PlatformSessionConnector::new(storage));
        let origin = connector.origin().to_owned();
        Self::configured(Backend::Platform(connector), &origin, max_in_flight)
    }

    fn configured(
        backend: Backend,
        origin: &str,
        max_in_flight: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        config::validate(origin, &[max_in_flight])?;
        Ok(Self {
            backend,
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
