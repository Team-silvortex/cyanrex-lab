use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Deserialize;

use crate::{models::module::ModuleInfo, AppState};

#[derive(Deserialize)]
pub struct ModuleControlRequest {
    pub name: String,
}

pub async fn list_modules(State(state): State<Arc<AppState>>) -> Json<Vec<ModuleInfo>> {
    Json(state.module_manager.list())
}

pub async fn start_module(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ModuleControlRequest>,
) -> Json<ModuleInfo> {
    Json(state.module_manager.start(&payload.name))
}

pub async fn stop_module(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ModuleControlRequest>,
) -> Json<ModuleInfo> {
    Json(state.module_manager.stop(&payload.name))
}
