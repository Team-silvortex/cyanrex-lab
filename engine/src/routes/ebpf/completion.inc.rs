pub async fn complete_ebpf(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EbpfCompletionRequest>,
) -> (StatusCode, Json<EbpfCompletionResponse>) {
    if payload.code.len() > MAX_EBPF_SOURCE_BYTES {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(EbpfCompletionResponse {
                ok: false,
                items: Vec::new(),
                message: format!("source exceeds {MAX_EBPF_SOURCE_BYTES} byte limit"),
            }),
        );
    }
    if payload.line == 0 || payload.column == 0 || payload.line > 100_000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(EbpfCompletionResponse {
                ok: false,
                items: Vec::new(),
                message: "invalid one-based cursor position".to_string(),
            }),
        );
    }

    let slots = EBPF_RUN_SLOTS.get_or_init(|| Semaphore::new(2));
    let Ok(_permit) = slots.try_acquire() else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(EbpfCompletionResponse {
                ok: false,
                items: Vec::new(),
                message: "compiler is busy; retry shortly".to_string(),
            }),
        );
    };

    (
        StatusCode::OK,
        Json(state.ebpf_loader.complete(&payload.code, payload.line, payload.column).await),
    )
}
