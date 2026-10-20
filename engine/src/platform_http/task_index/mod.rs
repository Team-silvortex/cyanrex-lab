//! Owner-private metadata over current-Session commands; explicit Source or retained-host backend.

mod contract;
mod handlers;

use super::{config, security, TaskContentHttpConfigError};
use crate::models::collaboration::{TaskRef, WorkspaceRef};
#[cfg(unix)]
use crate::platform_host::{PlatformStorageConnector, PlatformTaskIndexConnector};
use crate::services::auth_service::durable_source::{
    DurableAuthSource, SessionTaskContentIndexWorkspace, SessionTaskError,
};
use crate::services::task_store::TaskContentIndexPage;
use axum::{middleware, routing::any, Router};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Trusted composition only: no owner, namespace or catalogue selection from requests.
#[derive(Clone)]
pub struct TaskContentIndexHttpState {
    backend: Backend,
    origin: String,
    admission: Arc<Semaphore>,
}

#[derive(Clone)]
enum Backend {
    Source {
        source: DurableAuthSource,
        workspace: SessionTaskContentIndexWorkspace,
    },
    #[cfg(unix)]
    Platform(Arc<PlatformTaskIndexConnector>),
}

impl Backend {
    fn scope(&self) -> WorkspaceRef {
        match self {
            Self::Source { workspace, .. } => workspace.index_scope(),
            #[cfg(unix)]
            Self::Platform(connector) => connector.scope(),
        }
    }

    async fn list(
        &self,
        token: &str,
        after: Option<TaskRef>,
        limit: Option<usize>,
    ) -> Result<TaskContentIndexPage, SessionTaskError> {
        match self {
            Self::Source { source, workspace } => {
                source
                    .list_session_content_tasks(token, workspace, after, limit)
                    .await
            }
            #[cfg(unix)]
            Self::Platform(connector) => {
                connector
                    .list_session_content_tasks(token, after, limit)
                    .await
            }
        }
    }
}

impl TaskContentIndexHttpState {
    pub fn new(
        source: DurableAuthSource,
        workspace: SessionTaskContentIndexWorkspace,
        origin: &str,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacity(source, workspace, origin, 4)
    }

    /// Cloned states share slots through response-body completion/drop; separate states do not.
    pub fn with_capacity(
        source: DurableAuthSource,
        workspace: SessionTaskContentIndexWorkspace,
        origin: &str,
        max_in_flight: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::configured(Backend::Source { source, workspace }, origin, max_in_flight)
    }

    /// Retain the original storage only; derive scope, namespace and Origin without dialing.
    #[cfg(unix)]
    pub fn with_platform_storage(
        storage: PlatformStorageConnector,
        max_in_flight: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        config::validate(storage.origin(), &[max_in_flight])?;
        Self::with_platform_connector(PlatformTaskIndexConnector::new(storage), max_in_flight)
    }

    #[cfg(unix)]
    pub(super) fn with_platform_connector(
        connector: PlatformTaskIndexConnector,
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
            admission: Arc::new(Semaphore::new(max_in_flight)),
        })
    }
}

/// Standalone read-only router, never merged into the live application automatically.
pub fn build_task_content_index_router(state: TaskContentIndexHttpState) -> Router {
    routes()
        .fallback(handlers::dispatch)
        .layer(middleware::from_fn(security::response_headers))
        .with_state(state)
}

/// Fallback-free routes for an explicitly configured platform workspace host.
#[cfg_attr(not(unix), allow(dead_code))]
pub(super) fn routes() -> Router<TaskContentIndexHttpState> {
    Router::new().route("/platform/v1/task-index", any(handlers::dispatch))
}
