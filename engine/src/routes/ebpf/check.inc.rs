pub async fn check_ebpf(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EbpfRunRequest>,
) -> (StatusCode, Json<EbpfCheckResponse>) {
    if payload.code.len() > MAX_EBPF_SOURCE_BYTES {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(EbpfCheckResponse {
                ok: false,
                message: format!("source exceeds {MAX_EBPF_SOURCE_BYTES} byte limit"),
                diagnostics: Vec::new(),
                stdout: String::new(),
                stderr: String::new(),
            }),
        );
    }

    let slots = EBPF_RUN_SLOTS.get_or_init(|| Semaphore::new(2));
    let Ok(_permit) = slots.try_acquire() else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(EbpfCheckResponse {
                ok: false,
                message: "compiler is busy; retry shortly".to_string(),
                diagnostics: Vec::new(),
                stdout: String::new(),
                stderr: String::new(),
            }),
        );
    };

    (StatusCode::OK, Json(state.ebpf_loader.check(&payload.code).await))
}
