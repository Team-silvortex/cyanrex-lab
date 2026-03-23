use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::{
    models::c_headers::{
        DownloadHeaderRequest, HeaderModuleState, HeaderSelectionMetadata, ModuleActionResponse,
        SelectHeaderRequest,
    },
    AppState,
};

pub async fn list_headers(State(state): State<Arc<AppState>>) -> Json<HeaderModuleState> {
    Json(state.c_header_module.list().await)
}

pub async fn download_header(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DownloadHeaderRequest>,
) -> (StatusCode, Json<ModuleActionResponse>) {
    match state.c_header_module.download(&payload.id).await {
        Ok(message) => (
            StatusCode::OK,
            Json(ModuleActionResponse { ok: true, message }),
        ),
        Err(message) => (
            StatusCode::BAD_REQUEST,
            Json(ModuleActionResponse { ok: false, message }),
        ),
    }
}

pub async fn select_header(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SelectHeaderRequest>,
) -> (StatusCode, Json<ModuleActionResponse>) {
    match state
        .c_header_module
        .set_selected(&payload.id, payload.selected)
        .await
    {
        Ok(message) => (
            StatusCode::OK,
            Json(ModuleActionResponse { ok: true, message }),
        ),
        Err(message) => (
            StatusCode::BAD_REQUEST,
            Json(ModuleActionResponse { ok: false, message }),
        ),
    }
}

pub async fn selected_metadata(
    State(state): State<Arc<AppState>>,
) -> Json<HeaderSelectionMetadata> {
    Json(state.c_header_module.selected_metadata().await)
}

pub async fn delete_header(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DownloadHeaderRequest>,
) -> (StatusCode, Json<ModuleActionResponse>) {
    match state.c_header_module.delete(&payload.id).await {
        Ok(message) => (
            StatusCode::OK,
            Json(ModuleActionResponse { ok: true, message }),
        ),
        Err(message) => (
            StatusCode::BAD_REQUEST,
            Json(ModuleActionResponse { ok: false, message }),
        ),
    }
}
