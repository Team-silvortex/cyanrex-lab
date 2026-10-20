//! Explicit existing-account issuer only, never a listener or teaching-auth fallback.
mod handlers;
mod headers;
mod rate;
#[cfg(test)]
pub(super) mod test_support;
#[cfg(test)]
mod tests;

use super::{config, security, TaskContentHttpConfigError};
use crate::{
    models::collaboration::LegacyUsername,
    platform_host::PlatformLoginConnector,
    services::auth_service::durable_source::{DurableAuthError, DurableLogin},
};
use axum::{middleware, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Trusted composition only. Clones share the original issuer and both admission budgets.
#[derive(Clone)]
pub struct SessionLoginHttpState {
    backend: Backend,
    origin: String,
    rate: Arc<rate::LoginRate>,
    admission: Arc<Semaphore>,
}

#[derive(Clone)]
enum Backend {
    Platform(Arc<PlatformLoginConnector>),
    #[cfg(test)]
    Test(Arc<test_support::Backend>),
}

impl Backend {
    async fn login(
        &self,
        username: &LegacyUsername,
        password: &str,
        otp: &str,
    ) -> Result<DurableLogin, DurableAuthError> {
        match self {
            Self::Platform(connector) => connector.login(username, password, otp).await,
            #[cfg(test)]
            Self::Test(backend) => backend.login(username, password, otp).await,
        }
    }
}

impl SessionLoginHttpState {
    /// Pure composition of an explicit retained issuer. No connection or Session is created.
    pub fn new(connector: PlatformLoginConnector) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(connector, 4)
    }

    pub fn with_capacity(
        connector: PlatformLoginConnector,
        max_in_flight: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        let origin = connector.origin().to_owned();
        Self::configured(
            Backend::Platform(Arc::new(connector)),
            &origin,
            max_in_flight,
        )
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
            rate: Arc::new(rate::LoginRate::new()),
            admission: Arc::new(Semaphore::new(max_in_flight)),
        })
    }
}

/// Independent unmounted route; no automatic merge into a lifecycle or application router.
pub fn build_session_login_router(state: SessionLoginHttpState) -> Router {
    routes()
        .fallback(handlers::dispatch)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}

/// Exact private routes only; the chosen composition owns its fallback and response middleware.
pub(super) fn routes() -> Router<SessionLoginHttpState> {
    Router::new().route("/platform/v1/session/login", any(handlers::dispatch))
}
