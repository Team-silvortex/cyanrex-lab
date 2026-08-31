use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::{models::module::ModuleInfo, AppState};

#[derive(Deserialize)]
pub struct ModuleControlRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct ModuleControlError {
    pub ok: bool,
    pub message: String,
}

pub async fn list_modules(State(state): State<Arc<AppState>>) -> Json<Vec<ModuleInfo>> {
    Json(state.module_manager.list())
}

pub async fn start_module(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ModuleControlRequest>,
) -> Result<Json<ModuleInfo>, (StatusCode, Json<ModuleControlError>)> {
    state
        .module_manager
        .start(&payload.name)
        .map(Json)
        .map_err(module_not_found)
}

pub async fn stop_module(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ModuleControlRequest>,
) -> Result<Json<ModuleInfo>, (StatusCode, Json<ModuleControlError>)> {
    state
        .module_manager
        .stop(&payload.name)
        .map(Json)
        .map_err(module_not_found)
}

fn module_not_found(error: impl std::fmt::Display) -> (StatusCode, Json<ModuleControlError>) {
    (
        StatusCode::NOT_FOUND,
        Json(ModuleControlError {
            ok: false,
            message: error.to_string(),
        }),
    )
}
