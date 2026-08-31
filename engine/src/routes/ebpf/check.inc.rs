pub async fn check_ebpf(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EbpfRunRequest>,
) -> (StatusCode, Json<EbpfCheckResponse>) {
    state.record_check_request();
    let start = Instant::now();

    if payload.code.len() > MAX_EBPF_SOURCE_BYTES {
        let response = EbpfCheckResponse {
            ok: false,
            message: format!("source exceeds {MAX_EBPF_SOURCE_BYTES} byte limit"),
            diagnostics: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
        };
        state.finish_check_request(
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

    let slots = EBPF_CHECK_SLOTS.get_or_init(|| Semaphore::new(2));
    let Ok(_permit) = slots.try_acquire() else {
        let response = EbpfCheckResponse {
            ok: false,
            message: "compiler is busy; retry shortly".to_string(),
            diagnostics: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
        };
        state.finish_check_request(
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
        .check_with_cache_status(&payload.code, &selected_headers)
        .await;
    state.finish_check_request(
        start.elapsed().as_nanos() as u64,
        Some(cache_hit),
        response.ok,
        false,
    );
    (StatusCode::OK, Json(response))
}
