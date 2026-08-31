use std::sync::Arc;

use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};

use crate::AppState;

pub async fn status(State(state): State<Arc<AppState>>, headers: HeaderMap) -> impl IntoResponse {
    let username = crate::routes::auth::current_session_from_headers(state.as_ref(), &headers)
        .await
        .map(|session| session.username)
        .unwrap_or_default();
    Json(state.runner_manager.status_for(&username))
}

pub async fn overview(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(state.runner_manager.overview())
}
