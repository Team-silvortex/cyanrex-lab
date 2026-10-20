//! Retained-host owner-private empty Task creation, without content publication or execution.
use super::{config, errors::HttpError, security, TaskContentHttpConfigError};
use crate::{
    models::collaboration::TaskId,
    platform_host::PlatformTaskCreateConnector,
    services::{auth_service::durable_source::SessionTaskError, task_store::TaskContentSnapshot},
};
use axum::{http::StatusCode, middleware, response::IntoResponse, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

mod handlers;

#[cfg(test)]
mod admission_tests;
#[cfg(test)]
pub(super) mod test_support;
#[cfg(test)]
mod tests;

/// Trusted construction only. Clones share the original binding and response-lifetime budget.
#[derive(Clone)]
pub struct TaskCreateHttpState {
    backend: Backend,
    origin: String,
    admission: Arc<Semaphore>,
}

#[derive(Clone)]
enum Backend {
    Platform(Arc<PlatformTaskCreateConnector>),
    // Synthetic transport fixtures cannot be selected by production callers.
    #[cfg(test)]
    Test(Arc<test_support::Backend>),
}

impl Backend {
    async fn create(
        &self,
        token: &str,
        id: TaskId,
        title: String,
    ) -> Result<TaskContentSnapshot, SessionTaskError> {
        match self {
            Self::Platform(connector) => {
                connector
                    .create_session_empty_content_task(token, id, &title)
                    .await
            }
            #[cfg(test)]
            Self::Test(backend) => backend.create(token, id, title).await,
        }
    }
}

impl TaskCreateHttpState {
    pub fn new(connector: PlatformTaskCreateConnector) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(connector, 4)
    }

    pub fn with_capacity(
        connector: PlatformTaskCreateConnector,
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

/// Standalone empty-create capability only, never implicitly mounted in the teaching Engine.
pub fn build_task_create_router(state: TaskCreateHttpState) -> Router {
    routes()
        .fallback(not_found)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}

/// Fallback-free routes for explicit retained-host composition.
pub(super) fn routes() -> Router<TaskCreateHttpState> {
    Router::new().route("/platform/v1/tasks", any(handlers::dispatch))
}

async fn not_found() -> axum::response::Response {
    HttpError::new(StatusCode::NOT_FOUND, "not_found").into_response()
}
