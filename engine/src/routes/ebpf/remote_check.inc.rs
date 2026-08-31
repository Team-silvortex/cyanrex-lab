pub async fn check_backends(
    State(state): State<Arc<AppState>>,
) -> Json<EbpfCheckBackendInventory> {
    let agents = state
        .runner_agent_registry
        .inventory()
        .agents
        .into_iter()
        .filter(|agent| {
            agent.state == RunnerAgentState::Healthy
                && agent.isolation != RunnerAgentIsolation::SharedKernel
                && agent
                    .capabilities
                    .iter()
                    .any(|capability| capability == "clang_check")
        })
        .map(|agent| EbpfCheckBackend {
            agent_id: agent.agent_id,
            isolation: agent.isolation,
            state: agent.state,
            available_slots: agent.available_slots,
            max_concurrent: agent.max_concurrent,
        })
        .collect();
    Json(EbpfCheckBackendInventory {
        local_available: true,
        agents,
    })
}

pub async fn submit_remote_check(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<EbpfRemoteCheckSubmitRequest>,
) -> Response {
    let Some(username) = remote_check_username(state.as_ref(), &headers).await else {
        return remote_check_error(StatusCode::UNAUTHORIZED, "invalid auth session");
    };
    let Some(agent) = state.runner_agent_registry.agent(&request.agent_id) else {
        return remote_check_error(StatusCode::BAD_REQUEST, "remote compiler Agent is unknown");
    };
    if agent.state != RunnerAgentState::Healthy
        || agent.isolation == RunnerAgentIsolation::SharedKernel
        || !agent
            .capabilities
            .iter()
            .any(|capability| capability == "clang_check")
    {
        return remote_check_error(
            StatusCode::CONFLICT,
            "remote compiler Agent is unavailable or ineligible",
        );
    }
    match state.runner_job_queue.submit_user_compile_check(
        username,
        request.agent_id,
        request.code,
        request.program_name,
        Some(20),
    ) {
        Ok(job) => (
            StatusCode::ACCEPTED,
            Json(remote_check_response(&job)),
        )
            .into_response(),
        Err(error) => remote_check_queue_error(error),
    }
}

pub async fn remote_check_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<EbpfRemoteCheckStatusQuery>,
) -> Response {
    let Some(username) = remote_check_username(state.as_ref(), &headers).await else {
        return remote_check_error(StatusCode::UNAUTHORIZED, "invalid auth session");
    };
    match state
        .runner_job_queue
        .job_for_owner(&query.job_id, &username)
    {
        Ok(job) => Json(remote_check_response(&job)).into_response(),
        Err(error) => remote_check_queue_error(error),
    }
}

pub async fn cancel_remote_check(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<EbpfRemoteCheckCancelRequest>,
) -> Response {
    let Some(username) = remote_check_username(state.as_ref(), &headers).await else {
        return remote_check_error(StatusCode::UNAUTHORIZED, "invalid auth session");
    };
    match state
        .runner_job_queue
        .cancel_for_owner(&request.job_id, &username)
    {
        Ok(job) => Json(remote_check_response(&job)).into_response(),
        Err(error) => remote_check_queue_error(error),
    }
}

async fn remote_check_username(state: &AppState, headers: &HeaderMap) -> Option<String> {
    crate::routes::auth::current_session_from_headers(state, headers)
        .await
        .map(|session| session.username)
}

fn remote_check_response(job: &RunnerJobView) -> EbpfRemoteCheckResponse {
    let result = if remote_check_terminal(job.state) {
        Some(remote_check_result(job))
    } else {
        None
    };
    EbpfRemoteCheckResponse {
        job_id: job.job_id.clone(),
        state: job.state,
        agent_id: job
            .assigned_agent_id
            .clone()
            .or_else(|| job.target_agent_id.clone()),
        message: job
            .result_message
            .clone()
            .unwrap_or_else(|| job.message.clone()),
        result,
    }
}

fn remote_check_result(job: &RunnerJobView) -> EbpfCheckResponse {
    if let Some(report) = job
        .output
        .as_deref()
        .and_then(|output| serde_json::from_str::<RunnerCompileReport>(output).ok())
    {
        return EbpfCheckResponse {
            ok: job.state == RunnerJobState::Succeeded && report.success,
            message: job
                .result_message
                .clone()
                .unwrap_or_else(|| "remote compile check completed".to_string()),
            diagnostics: crate::services::ebpf_loader::parse_clang_diagnostics(&report.stderr),
            stdout: report.stdout,
            stderr: report.stderr,
        };
    }
    EbpfCheckResponse {
        ok: false,
        message: job.result_message.clone().unwrap_or_else(|| match job.state {
            RunnerJobState::Cancelled => "remote compile check cancelled".to_string(),
            RunnerJobState::Expired => "remote compile check expired".to_string(),
            _ => "remote compile check failed without a compiler report".to_string(),
        }),
        diagnostics: Vec::new(),
        stdout: String::new(),
        stderr: job.output.clone().unwrap_or_default(),
    }
}

fn remote_check_terminal(state: RunnerJobState) -> bool {
    matches!(
        state,
        RunnerJobState::Succeeded
            | RunnerJobState::Failed
            | RunnerJobState::Cancelled
            | RunnerJobState::Expired
    )
}

fn remote_check_queue_error(error: RunnerJobQueueError) -> Response {
    match error {
        RunnerJobQueueError::Invalid(message) => {
            remote_check_error(StatusCode::BAD_REQUEST, &message)
        }
        RunnerJobQueueError::NotFound => {
            remote_check_error(StatusCode::NOT_FOUND, "remote check was not found")
        }
        RunnerJobQueueError::Conflict(message) => {
            remote_check_error(StatusCode::CONFLICT, &message)
        }
        RunnerJobQueueError::Capacity => {
            remote_check_error(StatusCode::SERVICE_UNAVAILABLE, "remote check queue is full")
        }
    }
}

fn remote_check_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"ok": false, "message": message})),
    )
        .into_response()
}
