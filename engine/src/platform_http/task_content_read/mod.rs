//! Retained-host owner-private Task content reopening; no resource write or execution route.
use super::{config, errors::HttpError, security, TaskContentHttpConfigError};
use crate::{
    models::collaboration::TaskId,
    platform_host::PlatformTaskContentConnector,
    services::auth_service::durable_source::{SessionTaskContent, SessionTaskError},
};
use axum::{http::StatusCode, middleware, response::IntoResponse, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

mod handlers;

#[cfg(test)]
pub(super) mod test_support;
#[cfg(test)]
mod tests;

/// Trusted construction only; the selected connector retains the original storage binding.
#[derive(Clone)]
pub struct TaskContentReadHttpState {
    backend: Backend,
    origin: String,
    admission: Arc<Semaphore>,
}

#[derive(Clone)]
enum Backend {
    Platform(Arc<PlatformTaskContentConnector>),
    // Transport-only closed Source fixtures. No production fallback or alternate constructor.
    #[cfg(test)]
    Source {
        source: crate::services::auth_service::durable_source::DurableAuthSource,
        workspace: crate::services::auth_service::durable_source::SessionTaskContentWorkspace,
        _root: Arc<test_support::TestRoot>,
    },
}

impl Backend {
    async fn read(
        &self,
        token: &str,
        id: TaskId,
    ) -> Result<Option<SessionTaskContent>, SessionTaskError> {
        match self {
            Self::Platform(connector) => connector.get_session_content_task(token, id).await,
            #[cfg(test)]
            Self::Source {
                source, workspace, ..
            } => source.get_session_content_task(token, workspace, id).await,
        }
    }
}

impl TaskContentReadHttpState {
    pub fn new(
        connector: PlatformTaskContentConnector,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(connector, 4)
    }

    pub fn with_capacity(
        connector: PlatformTaskContentConnector,
        max_in_flight: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        config::validate(connector.origin(), &[max_in_flight])?;
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
            admission: Arc::new(Semaphore::new(max_in_flight)),
        })
    }
}

/// Standalone read-only composition, never implicitly mounted in the teaching Engine router.
pub fn build_task_content_read_router(state: TaskContentReadHttpState) -> Router {
    routes()
        .fallback(not_found)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}

/// Fallback-free routes for explicit retained-host composition.
pub(super) fn routes() -> Router<TaskContentReadHttpState> {
    Router::new().route("/platform/v1/tasks/{task_id}", any(handlers::dispatch))
}

async fn not_found() -> axum::response::Response {
    HttpError::new(StatusCode::NOT_FOUND, "not_found").into_response()
}
