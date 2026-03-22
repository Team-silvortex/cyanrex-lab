use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::{
    models::ebpf::{EbpfRunRequest, EbpfRunResponse},
    AppState,
};

pub async fn run_ebpf(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EbpfRunRequest>,
) -> (StatusCode, Json<EbpfRunResponse>) {
    let result = state.ebpf_loader.run(&payload.code).await;

    let status = if result.stage == "validation" {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::OK
    };

    (status, Json(result))
}
