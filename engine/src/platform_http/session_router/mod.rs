//! One retained binding and independent login/lifecycle HTTP budgets, never a public listener.
use super::{
    config, errors::HttpError, security, session_lifecycle, session_login,
    SessionLifecycleHttpState, SessionLoginHttpState, TaskContentHttpConfigError,
};
use crate::platform_host::{split_platform_session_connectors, PlatformStorageConnector};
use axum::{
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    Router,
};

#[cfg(test)]
mod tests;

/// Trusted construction only. Clones share each separate budget and the same original binding.
#[derive(Clone)]
pub struct PlatformSessionHttpState {
    login: SessionLoginHttpState,
    lifecycle: SessionLifecycleHttpState,
}

impl PlatformSessionHttpState {
    pub fn new(storage: PlatformStorageConnector) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacities(storage, 4, 4)
    }

    pub fn with_capacities(
        storage: PlatformStorageConnector,
        login_capacity: usize,
        lifecycle_capacity: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        // Validate all public metadata before allocating shared storage, a LoginContext or either
        // HTTP budget. No independently supplied Origin/source can mismatch the retained binding.
        config::validate(storage.origin(), &[login_capacity, lifecycle_capacity])?;
        let (login, lifecycle) = split_platform_session_connectors(storage);
        Ok(Self {
            login: SessionLoginHttpState::with_capacity(login, login_capacity)?,
            lifecycle: SessionLifecycleHttpState::with_platform_connector(
                lifecycle,
                lifecycle_capacity,
            )?,
        })
    }
}

/// Explicit unmounted composition, not an application merge, listener or TLS acceptance proof.
pub fn build_platform_session_router(state: PlatformSessionHttpState) -> Router {
    Router::new()
        // The private route sets have no fallback or layers; do not merge the standalone routers.
        .merge(session_login::routes().with_state(state.login))
        .merge(session_lifecycle::routes().with_state(state.lifecycle))
        .fallback(not_found)
        .layer(middleware::from_fn(security::response_headers))
}

async fn not_found() -> Response {
    HttpError::new(StatusCode::NOT_FOUND, "not_found").into_response()
}
