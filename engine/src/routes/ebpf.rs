use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};

use crate::{
    models::ebpf::{EbpfRunRequest, EbpfRunResponse},
    AppState,
};

const MAX_EBPF_SOURCE_BYTES: usize = 256 * 1024;

pub async fn run_ebpf(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EbpfRunRequest>,
) -> (StatusCode, Json<EbpfRunResponse>) {
    if let Some(validation_error) = validate_ebpf_source(&payload.code) {
        return (StatusCode::BAD_REQUEST, Json(validation_error));
    }

    let result = state.ebpf_loader.run(&payload.code).await;

    let status = if result.stage == "validation" {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::OK
    };

    (status, Json(result))
}

fn validate_ebpf_source(code: &str) -> Option<EbpfRunResponse> {
    if code.trim().is_empty() {
        return Some(EbpfRunResponse::validation_error(
            "eBPF source code is empty",
        ));
    }

    if code.len() > MAX_EBPF_SOURCE_BYTES {
        return Some(EbpfRunResponse::validation_error(format!(
            "eBPF source exceeds {} bytes",
            MAX_EBPF_SOURCE_BYTES
        )));
    }

    if code.contains('\0') {
        return Some(EbpfRunResponse::validation_error(
            "eBPF source contains unsupported null byte",
        ));
    }

    None
}
