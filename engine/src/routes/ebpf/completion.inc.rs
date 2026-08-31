pub async fn complete_ebpf(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EbpfCompletionRequest>,
) -> (StatusCode, Json<EbpfCompletionResponse>) {
    state.record_completion_request();
    let start = Instant::now();

    if payload.code.len() > MAX_EBPF_SOURCE_BYTES {
        let response = EbpfCompletionResponse {
            ok: false,
            items: Vec::new(),
            message: format!("source exceeds {MAX_EBPF_SOURCE_BYTES} byte limit"),
        };
        state.finish_completion_request(
            start.elapsed().as_nanos() as u64,
            None,
            response.ok,
            false,
        );
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(response),
        );
    }
    if payload.line == 0 || payload.column == 0 || payload.line > 100_000 {
        let response = EbpfCompletionResponse {
            ok: false,
            items: Vec::new(),
            message: "invalid one-based cursor position".to_string(),
        };
        state.finish_completion_request(
            start.elapsed().as_nanos() as u64,
            None,
            response.ok,
            false,
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(response),
        );
    }

    let slots = EBPF_COMPLETION_SLOTS.get_or_init(|| Semaphore::new(3));
    let Ok(_permit) = slots.try_acquire() else {
        let response = EbpfCompletionResponse {
            ok: false,
            items: Vec::new(),
            message: "compiler is busy; retry shortly".to_string(),
        };
        state.finish_completion_request(
            start.elapsed().as_nanos() as u64,
            None,
            response.ok,
            true,
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(response),
        );
    };
    let selected_headers = state
        .c_header_module
        .selected_metadata()
        .await
        .selected_headers;

    let (response, cache_hit) = state
        .ebpf_loader
        .complete_with_cache_status(
            &payload.code,
            payload.line,
            payload.column,
            &selected_headers,
        )
        .await;
    state.finish_completion_request(
        start.elapsed().as_nanos() as u64,
        Some(cache_hit),
        response.ok,
        false,
    );
    (
        StatusCode::OK,
        Json(response),
    )
}
