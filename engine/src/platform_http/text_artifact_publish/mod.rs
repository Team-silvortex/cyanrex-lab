//! Explicit retained new-text transport, separate from pooled reads and never implicitly mounted.
use super::{config, errors::HttpError, security, TaskContentHttpConfigError};
use crate::{
    models::collaboration::{ArtifactId, ArtifactRevision, ArtifactRevisionId},
    platform_host::PlatformTextArtifactConnector,
    services::auth_service::durable_source::SessionArtifactError,
};
use axum::{http::StatusCode, middleware, response::IntoResponse, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[cfg(test)]
mod admission_tests;
mod handlers;
#[cfg(test)]
pub(super) mod test_support;
#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct TextArtifactPublishHttpState {
    backend: Backend,
    origin: String,
    admission: Arc<Semaphore>,
}

#[derive(Clone)]
enum Backend {
    Platform(Arc<PlatformTextArtifactConnector>),
    #[cfg(test)]
    Test(Arc<test_support::Backend>),
}

impl Backend {
    async fn create(
        &self,
        token: &str,
        artifact: ArtifactId,
        revision: ArtifactRevisionId,
        title: String,
        text: String,
    ) -> Result<ArtifactRevision, SessionArtifactError> {
        match self {
            Self::Platform(connector) => {
                connector
                    .create_session_text_artifact(token, artifact, revision, title, text)
                    .await
            }
            #[cfg(test)]
            Self::Test(backend) => backend.create(token, artifact, revision, title, text).await,
        }
    }
}

impl TextArtifactPublishHttpState {
    pub fn new(
        connector: PlatformTextArtifactConnector,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(connector, 4)
    }

    pub fn with_capacity(
        connector: PlatformTextArtifactConnector,
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

/// One POST-only capability. No implicit merge into any host, teaching or pooled router.
pub fn build_text_artifact_publish_router(state: TextArtifactPublishHttpState) -> Router {
    routes()
        .fallback(not_found)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}

pub(super) fn routes() -> Router<TextArtifactPublishHttpState> {
    Router::new().route("/platform/v1/text-artifacts", any(handlers::dispatch))
}

async fn not_found() -> axum::response::Response {
    HttpError::new(StatusCode::NOT_FOUND, "not_found").into_response()
}
