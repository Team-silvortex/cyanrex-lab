//! Explicit Task-bound text transport, not a Task save or an implicitly mounted host route.
use super::{config, errors::HttpError, security, TaskContentHttpConfigError};
use crate::{
    models::collaboration::{ArtifactId, ArtifactRevision, ArtifactRevisionId, WorkspaceRef},
    platform_host::PlatformTaskTextArtifactConnector,
    services::{auth_service::durable_source::SessionTaskError, task_store::TaskContentSnapshot},
};
use axum::{http::StatusCode, middleware, response::IntoResponse, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[cfg(test)]
mod admission_tests;
mod contract;
#[cfg(test)]
mod contract_tests;
mod errors;
mod handlers;
#[cfg(test)]
pub(super) mod test_support;
#[cfg(test)]
mod tests;

/// Trusted construction only. Clones share the original connector and response-lifetime slots.
#[derive(Clone)]
pub struct TaskTextArtifactPublishHttpState {
    backend: Backend,
    origin: String,
    scope: WorkspaceRef,
    admission: Arc<Semaphore>,
}

#[derive(Clone)]
enum Backend {
    Platform(Arc<PlatformTaskTextArtifactConnector>),
    #[cfg(test)]
    Test(Arc<test_support::Backend>),
}

impl Backend {
    async fn create(
        &self,
        token: &str,
        expected: TaskContentSnapshot,
        artifact: ArtifactId,
        revision: ArtifactRevisionId,
        text: String,
    ) -> Result<ArtifactRevision, SessionTaskError> {
        match self {
            Self::Platform(connector) => {
                connector
                    .create_session_task_text_artifact(token, expected, artifact, revision, text)
                    .await
            }
            #[cfg(test)]
            Self::Test(backend) => {
                backend
                    .create(token, expected, artifact, revision, text)
                    .await
            }
        }
    }
}

impl TaskTextArtifactPublishHttpState {
    pub fn new(
        connector: PlatformTaskTextArtifactConnector,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(connector, 4)
    }

    pub fn with_capacity(
        connector: PlatformTaskTextArtifactConnector,
        max_in_flight: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        config::validate(connector.origin(), &[max_in_flight])?;
        let origin = connector.origin().to_owned();
        let scope = connector.scope();
        Self::configured(
            Backend::Platform(Arc::new(connector)),
            &origin,
            scope,
            max_in_flight,
        )
    }

    fn configured(
        backend: Backend,
        origin: &str,
        scope: WorkspaceRef,
        max_in_flight: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        config::validate(origin, &[max_in_flight])?;
        Ok(Self {
            backend,
            origin: origin.to_owned(),
            scope,
            admission: Arc::new(Semaphore::new(max_in_flight)),
        })
    }
}

/// Standalone POST only. Workspace and CLI inclusion require separate explicit opt-in.
pub fn build_task_text_artifact_publish_router(state: TaskTextArtifactPublishHttpState) -> Router {
    routes()
        .fallback(not_found)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}

/// Fallback-free route for explicitly selected original-storage workspace composition.
pub(super) fn routes() -> Router<TaskTextArtifactPublishHttpState> {
    Router::new().route(
        "/platform/v1/tasks/{task_id}/text-artifacts",
        any(handlers::dispatch),
    )
}

async fn not_found() -> axum::response::Response {
    HttpError::new(StatusCode::NOT_FOUND, "not_found").into_response()
}
