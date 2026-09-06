pub async fn check_ebpf(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<EbpfRunRequest>,
) -> (StatusCode, Json<EbpfCheckResponse>) {
    let Some(session) =
        crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await
    else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(check_failure_response("authentication required")),
        );
    };
    let metrics = CompilerMetricsGuard::new(state.as_ref(), CompilerOperation::Check);
    if payload.code.len() > MAX_EBPF_SOURCE_BYTES {
        metrics.finish(None, false, false);
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(check_failure_response(format!(
                "source exceeds {MAX_EBPF_SOURCE_BYTES} byte limit",
            ))),
        );
    }
    let selected_headers = state
        .c_header_module
        .selected_metadata()
        .await
        .selected_headers;
    match state
        .runner_manager
        .check(RunnerCheckRequest {
            owner_username: &session.username,
            code: &payload.code,
            selected_headers: &selected_headers,
        })
        .await
    {
        Ok(outcome) => {
            metrics.finish(outcome.cache_hit, outcome.response.ok, false);
            (StatusCode::OK, Json(outcome.response))
        }
        Err(error) => {
            metrics.finish(None, false, matches!(error, RunnerOperationError::Busy));
            (
                runner_operation_status(&error),
                Json(check_failure_response(error.to_string())),
            )
        }
    }
}
