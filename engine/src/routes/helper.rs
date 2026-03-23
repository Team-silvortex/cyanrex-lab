use std::sync::Arc;

use axum::{extract::State, Json};

use crate::{models::environment::EnvironmentReport, AppState};

pub async fn environment_report(State(state): State<Arc<AppState>>) -> Json<EnvironmentReport> {
    Json(state.environment_checker.inspect().await)
}
