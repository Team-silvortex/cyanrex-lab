//! One retained storage binding for Session commands and private Task content commands.
use super::{
    config, errors::HttpError, security, session_lifecycle, session_login, task_content_read,
    task_content_replace, task_create, task_index, SessionLifecycleHttpState,
    SessionLoginHttpState, TaskContentHttpConfigError, TaskContentIndexHttpState,
    TaskContentReadHttpState, TaskContentReplaceHttpState, TaskCreateHttpState,
};
use crate::platform_host::{split_platform_task_replace_connectors, PlatformStorageConnector};
use axum::{
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    Router,
};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

/// Trusted construction only. Clones share one original binding and six independent budgets.
#[derive(Clone)]
pub struct PlatformWorkspaceHttpState {
    login: SessionLoginHttpState,
    lifecycle: SessionLifecycleHttpState,
    index: TaskContentIndexHttpState,
    content: TaskContentReadHttpState,
    create: TaskCreateHttpState,
    replace: TaskContentReplaceHttpState,
}

impl PlatformWorkspaceHttpState {
    pub fn new(storage: PlatformStorageConnector) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_capacities(storage, 4, 4, 4)
    }

    pub fn with_capacities(
        storage: PlatformStorageConnector,
        login_capacity: usize,
        lifecycle_capacity: usize,
        index_capacity: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_content_capacities(
            storage,
            login_capacity,
            lifecycle_capacity,
            index_capacity,
            4,
        )
    }

    /// Configure the content-read lane without changing the original three-budget constructor.
    pub fn with_content_capacities(
        storage: PlatformStorageConnector,
        login_capacity: usize,
        lifecycle_capacity: usize,
        index_capacity: usize,
        content_capacity: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_create_capacities(
            storage,
            login_capacity,
            lifecycle_capacity,
            index_capacity,
            content_capacity,
            4,
        )
    }

    /// Add an independently bounded empty-Task create lane, preserving earlier constructors.
    pub fn with_create_capacities(
        storage: PlatformStorageConnector,
        login_capacity: usize,
        lifecycle_capacity: usize,
        index_capacity: usize,
        content_capacity: usize,
        create_capacity: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        Self::with_replace_capacities(
            storage,
            login_capacity,
            lifecycle_capacity,
            index_capacity,
            content_capacity,
            create_capacity,
            4,
        )
    }

    /// Add a separately bounded manifest-replacement lane, preserving earlier constructors.
    pub fn with_replace_capacities(
        storage: PlatformStorageConnector,
        login_capacity: usize,
        lifecycle_capacity: usize,
        index_capacity: usize,
        content_capacity: usize,
        create_capacity: usize,
        replace_capacity: usize,
    ) -> Result<Self, TaskContentHttpConfigError> {
        // Validate every supplied capacity before allocating the shared binding or any lane.
        // Origin, namespaces and scope can only come from this same original storage handle.
        config::validate(
            storage.origin(),
            &[
                login_capacity,
                lifecycle_capacity,
                index_capacity,
                content_capacity,
                create_capacity,
                replace_capacity,
            ],
        )?;
        let (login, lifecycle, index, content, create, replace) =
            split_platform_task_replace_connectors(storage);
        Ok(Self {
            login: SessionLoginHttpState::with_capacity(login, login_capacity)?,
            lifecycle: SessionLifecycleHttpState::with_platform_connector(
                lifecycle,
                lifecycle_capacity,
            )?,
            index: TaskContentIndexHttpState::with_platform_connector(index, index_capacity)?,
            content: TaskContentReadHttpState::with_capacity(content, content_capacity)?,
            create: TaskCreateHttpState::with_capacity(create, create_capacity)?,
            replace: TaskContentReplaceHttpState::with_capacity(replace, replace_capacity)?,
        })
    }
}

/// Explicit composition only; no teaching AppState, payload publication, execution or listener.
pub fn build_platform_workspace_router(state: PlatformWorkspaceHttpState) -> Router {
    Router::new()
        .merge(session_login::routes().with_state(state.login))
        .merge(session_lifecycle::routes().with_state(state.lifecycle))
        .merge(task_index::routes().with_state(state.index))
        .merge(task_content_read::routes().with_state(state.content))
        .merge(task_create::routes().with_state(state.create))
        .merge(task_content_replace::routes().with_state(state.replace))
        .fallback(not_found)
        .layer(middleware::from_fn(security::response_headers))
}

async fn not_found() -> Response {
    HttpError::new(StatusCode::NOT_FOUND, "not_found").into_response()
}
