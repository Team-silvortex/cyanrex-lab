pub async fn complete_ebpf(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<EbpfCompletionRequest>,
) -> (StatusCode, Json<EbpfCompletionResponse>) {
    let Some(session) =
        crate::routes::auth::current_session_from_headers(state.as_ref(), &headers).await
    else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(completion_failure_response("authentication required")),
        );
    };
    let metrics = CompilerMetricsGuard::new(state.as_ref(), CompilerOperation::Completion);
    if payload.code.len() > MAX_EBPF_SOURCE_BYTES {
        metrics.finish(None, false, false);
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(completion_failure_response(format!(
                "source exceeds {MAX_EBPF_SOURCE_BYTES} byte limit",
            ))),
        );
    }
    if payload.line == 0 || payload.column == 0 || payload.line > 100_000 {
        metrics.finish(None, false, false);
        return (
            StatusCode::BAD_REQUEST,
            Json(completion_failure_response(
                "invalid one-based cursor position",
            )),
        );
    }
    let selected_headers = state
        .c_header_module
        .selected_metadata()
        .await
        .selected_headers;
    match state
        .runner_manager
        .complete(RunnerCompletionRequest {
            owner_username: &session.username,
            code: &payload.code,
            line: payload.line,
            column: payload.column,
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
                Json(completion_failure_response(error.to_string())),
            )
        }
    }
}
