pub async fn run_ebpf(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<EbpfRunRequest>,
) -> (StatusCode, Json<EbpfRunResponse>) {
    let username = crate::routes::auth::current_session_from_headers(state.as_ref(), &headers)
        .await
        .map(|session| session.username)
        .unwrap_or_else(|| "unknown".to_string());

    let program_name = payload
        .program_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("custom");
    let template_id = payload.template_id.clone();

    let sample_per_sec = payload.sampling_per_sec.unwrap_or(20).clamp(1, 200);
    let stream_seconds = payload.stream_seconds.unwrap_or(10).clamp(1, 120);
    let enable_kernel_stream = payload.enable_kernel_stream.unwrap_or(true);
    let mut runtime_backend = payload.runtime_backend.unwrap_or(EbpfRuntimeBackend::Bpftool);
    let selected_headers = state
        .c_header_module
        .selected_metadata()
        .await
        .selected_headers;

    if let Some(validation_error) = validate_ebpf_source(&payload.code) {
        state
            .event_bus
            .publish(Event {
                username: username.clone(),
                timestamp: Utc::now(),
                source: "module-ebpf".to_string(),
                event_type: "ebpf.validation_failed".to_string(),
                category: EventCategory::Platform,
                severity: EventSeverity::Warning,
                color: EventSeverity::Warning.color(),
                payload: json!({
                    "message": validation_error.message,
                }),
            })
            .await;
        return (StatusCode::BAD_REQUEST, Json(validation_error));
    }

    state
        .event_bus
        .publish(Event {
            username: username.clone(),
            timestamp: Utc::now(),
            source: "module-ebpf".to_string(),
            event_type: "ebpf.run_started".to_string(),
            category: EventCategory::Platform,
            severity: EventSeverity::Success,
            color: EventSeverity::Success.color(),
            payload: json!({
                "source_bytes": payload.code.len(),
                "program_name": program_name,
                "template_id": template_id.clone(),
                "runtime_backend": runtime_backend,
                "debug_breakpoints": payload.debug_breakpoints.clone().unwrap_or_default(),
            }),
        })
        .await;

    let slots = EBPF_RUN_SLOTS.get_or_init(|| Semaphore::new(2));
    let Ok(_permit) = slots.try_acquire() else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(EbpfRunResponse::validation_error(
                "too many eBPF jobs are already running",
            )),
        );
    };
    let mut result = match tokio::time::timeout(
        EBPF_EXECUTION_TIMEOUT,
        state
            .ebpf_loader
            .run(
                &username,
                &payload.code,
                Some(program_name),
                runtime_backend,
                &selected_headers,
                payload.debug_breakpoints.as_deref(),
            ),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            return (
                StatusCode::REQUEST_TIMEOUT,
                Json(EbpfRunResponse::validation_error(
                    "eBPF job exceeded the 45 second execution limit",
                )),
            )
        }
    };

    if should_attempt_aya_debug_fallback(&payload.code, runtime_backend, &result) {
        let bpftool_attach = verify_attach_state(
            result.pin_path.as_deref(),
            &payload.code,
            &format!("{}\n{}", result.load_stdout, result.load_stderr),
            &result.message,
            runtime_backend,
        )
        .await;
        let tooling_unavailable = bpftool_attach
            .reason
            .contains("both autoattach and manual tracepoint attach are unsupported");

        if !bpftool_attach.attached && tooling_unavailable {
            let previous_pin = result.pin_path.clone();
            let detached = state
                .ebpf_loader
                .detach_for_user(&username, previous_pin.as_deref())
                .await;

            if detached.is_ok() {
                runtime_backend = EbpfRuntimeBackend::Aya;
                result = match tokio::time::timeout(
                    EBPF_EXECUTION_TIMEOUT,
                    state.ebpf_loader.run(
                        &username,
                        &payload.code,
                        Some(program_name),
                        runtime_backend,
                        &selected_headers,
                        payload.debug_breakpoints.as_deref(),
                    ),
                )
                .await
                {
                    Ok(aya_result) => aya_result,
                    Err(_) => EbpfRunResponse {
                        success: false,
                        stage: "load".to_string(),
                        message: "Aya debug fallback exceeded the execution limit".to_string(),
                        compile_stdout: result.compile_stdout.clone(),
                        compile_stderr: result.compile_stderr.clone(),
                        load_stdout: String::new(),
                        load_stderr: String::new(),
                        pin_path: None,
                        debug: result.debug.clone(),
                    },
                };

                state
                    .event_bus
                    .publish(Event {
                        username: username.clone(),
                        timestamp: Utc::now(),
                        source: "module-ebpf".to_string(),
                        event_type: "ebpf.debug_backend_fallback".to_string(),
                        category: EventCategory::Platform,
                        severity: if result.success {
                            EventSeverity::Success
                        } else {
                            EventSeverity::Warning
                        },
                        color: if result.success {
                            EventSeverity::Success.color()
                        } else {
                            EventSeverity::Warning.color()
                        },
                        payload: json!({
                            "message": "bpftool tracepoint attach is unavailable; retried debug run with Aya",
                            "success": result.success,
                            "previous_pin_path": previous_pin,
                            "program_name": program_name,
                            "template_id": template_id.clone(),
                        }),
                    })
                    .await;
            }
        }
    }

    let mut attach_verified = false;
    let mut attach_expected = false;
    let debug_session_id = result
        .debug
        .as_ref()
        .and_then(|debug| debug.session_id.clone());
    let debug_trace_enabled = result
        .debug
        .as_ref()
        .is_some_and(|debug| !debug.instrumented_lines.is_empty());

    if result.success {
        let attach_check = verify_attach_state(
            result.pin_path.as_deref(),
            &payload.code,
            &format!("{}\n{}", result.load_stdout, result.load_stderr),
            &result.message,
            runtime_backend,
        )
        .await;
        let expect_attach = expects_autoattach(&payload.code);
        attach_expected = expect_attach;
        attach_verified = attach_check.attached;
        let attach_tooling_unavailable = attach_check
            .reason
            .contains("both autoattach and manual tracepoint attach are unsupported");
        let (event_type, severity, message) = if attach_check.attached {
            (
                "ebpf.attach_verified",
                EventSeverity::Success,
                "eBPF attachment verified".to_string(),
            )
        } else if expect_attach && attach_tooling_unavailable {
            (
                "ebpf.attach_unavailable",
                EventSeverity::Warning,
                "eBPF loaded but current bpftool cannot attach this program type".to_string(),
            )
        } else if expect_attach {
            (
                "ebpf.attach_missing",
                EventSeverity::Warning,
                "eBPF loaded but no active link was detected".to_string(),
            )
        } else {
            (
                "ebpf.attach_not_applicable",
                EventSeverity::Success,
                "program type may require manual attach target; autoattach verification skipped"
                    .to_string(),
            )
        };

        state
            .event_bus
            .publish(Event {
                username: username.clone(),
                timestamp: Utc::now(),
                source: "module-ebpf".to_string(),
                event_type: event_type.to_string(),
                category: EventCategory::Platform,
                severity,
                color: severity.color(),
                payload: json!({
                    "message": message,
                    "pin_path": result.pin_path.clone(),
                    "program_name": program_name,
                    "template_id": template_id.clone(),
                    "runtime_backend": runtime_backend,
                    "expected_autoattach": expect_attach,
                    "attached": attach_check.attached,
                    "reason": attach_check.reason,
                    "program_ids": attach_check.program_ids,
                    "linked_program_ids": attach_check.linked_program_ids,
                }),
            })
            .await;
    }

    if result.success && (enable_kernel_stream || debug_trace_enabled) && attach_verified {
        let event_bus = state.event_bus.clone();
        let ebpf_loader = state.ebpf_loader.clone();
        let username_for_stream = username.clone();
        let program_name_for_stream = program_name.to_string();
        let template_id_for_stream = template_id.clone();
        let pin_path = result.pin_path.clone();
        let code = payload.code.clone();
        let runtime_backend_for_stream = runtime_backend;
        let debug_session_id_for_stream = debug_session_id.clone();
        tokio::spawn(async move {
            stream_kernel_events(
                ebpf_loader,
                event_bus,
                username_for_stream,
                program_name_for_stream,
                template_id_for_stream,
                code,
                pin_path,
                runtime_backend_for_stream,
                sample_per_sec,
                stream_seconds,
                debug_session_id_for_stream,
            )
            .await;
        });
    } else if result.success
        && (enable_kernel_stream || debug_trace_enabled)
        && attach_expected
        && !attach_verified
    {
        state
            .event_bus
            .publish(Event {
                username: username.clone(),
                timestamp: Utc::now(),
                source: "module-ebpf".to_string(),
                event_type: "ebpf.kernel_stream_skipped".to_string(),
                category: EventCategory::Platform,
                severity: EventSeverity::Warning,
                color: EventSeverity::Warning.color(),
                payload: json!({
                    "message": "Kernel stream skipped because no active attach was detected",
                    "program_name": program_name,
                    "template_id": template_id.clone(),
                    "runtime_backend": runtime_backend,
                    "debug_session_id": debug_session_id,
                }),
            })
            .await;
    }

    state
        .event_bus
        .publish(Event {
            username,
            timestamp: Utc::now(),
            source: "module-ebpf".to_string(),
            event_type: "ebpf.run_finished".to_string(),
            category: EventCategory::Platform,
            severity: if result.success {
                EventSeverity::Success
            } else if result.stage == "compile" {
                EventSeverity::Warning
            } else {
                EventSeverity::Error
            },
            color: if result.success {
                EventSeverity::Success.color()
            } else if result.stage == "compile" {
                EventSeverity::Warning.color()
            } else {
                EventSeverity::Error.color()
            },
            payload: json!({
                "success": result.success,
                "stage": result.stage.clone(),
                "message": result.message.clone(),
                "program_name": program_name,
                "template_id": template_id,
                "runtime_backend": runtime_backend,
                "debug": result.debug.clone(),
            }),
        })
        .await;

    let status = if result.stage == "validation" {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::OK
    };

    (status, Json(result))
}

fn should_attempt_aya_debug_fallback(
    code: &str,
    runtime_backend: EbpfRuntimeBackend,
    result: &EbpfRunResponse,
) -> bool {
    runtime_backend == EbpfRuntimeBackend::Bpftool
        && result.success
        && result.pin_path.is_some()
        && code.contains("SEC(\"tracepoint/")
        && result
            .debug
            .as_ref()
            .is_some_and(|debug| !debug.instrumented_lines.is_empty())
}

#[cfg(test)]
mod debug_backend_fallback_tests {
    use super::should_attempt_aya_debug_fallback;
    use crate::models::ebpf::{EbpfDebugInfo, EbpfRunResponse, EbpfRuntimeBackend};

    fn successful_result(lines: Vec<u32>) -> EbpfRunResponse {
        EbpfRunResponse {
            success: true,
            stage: "run".to_string(),
            message: String::new(),
            compile_stdout: String::new(),
            compile_stderr: String::new(),
            load_stdout: String::new(),
            load_stderr: String::new(),
            pin_path: Some("/sys/fs/bpf/cyanrex/test".to_string()),
            debug: Some(EbpfDebugInfo {
                mode: "kernel-trace".to_string(),
                session_id: Some("session".to_string()),
                requested_lines: lines.clone(),
                instrumented_lines: lines,
                rejected: Vec::new(),
            }),
        }
    }

    #[test]
    fn fallback_requires_bpftool_tracepoint_and_active_debug_probe() {
        let tracepoint = "SEC(\"tracepoint/syscalls/sys_enter_execve\")";
        assert!(should_attempt_aya_debug_fallback(
            tracepoint,
            EbpfRuntimeBackend::Bpftool,
            &successful_result(vec![3]),
        ));
        assert!(!should_attempt_aya_debug_fallback(
            tracepoint,
            EbpfRuntimeBackend::Aya,
            &successful_result(vec![3]),
        ));
        assert!(!should_attempt_aya_debug_fallback(
            "SEC(\"xdp\")",
            EbpfRuntimeBackend::Bpftool,
            &successful_result(vec![3]),
        ));
        assert!(!should_attempt_aya_debug_fallback(
            tracepoint,
            EbpfRuntimeBackend::Bpftool,
            &successful_result(Vec::new()),
        ));
        let mut missing_pin = successful_result(vec![3]);
        missing_pin.pin_path = None;
        assert!(!should_attempt_aya_debug_fallback(
            tracepoint,
            EbpfRuntimeBackend::Bpftool,
            &missing_pin,
        ));
    }
}

pub async fn list_templates() -> Json<Vec<EbpfTemplate>> {
    Json(default_templates())
}

pub async fn list_attachments(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<EbpfAttachmentListResponse> {
    let username = crate::routes::auth::current_session_from_headers(state.as_ref(), &headers)
        .await
        .map(|session| session.username)
        .unwrap_or_else(|| "unknown".to_string());
    Json(EbpfAttachmentListResponse {
        pin_paths: state.ebpf_loader.list_attachments_for_user(&username).await,
    })
}

pub async fn list_attachment_details(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Json<EbpfAttachmentDetailListResponse> {
    let username = crate::routes::auth::current_session_from_headers(state.as_ref(), &headers)
        .await
        .map(|session| session.username)
        .unwrap_or_else(|| "unknown".to_string());
    let attachments = state
        .ebpf_loader
        .list_attachment_details_for_user(&username)
        .await
        .into_iter()
        .map(|(pin_path, source, program_name)| EbpfAttachmentDetail {
            pin_path,
            source,
            program_name,
        })
        .collect();

    Json(EbpfAttachmentDetailListResponse { attachments })
}

pub async fn detach_ebpf(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<EbpfDetachRequest>,
) -> (StatusCode, Json<EbpfDetachResponse>) {
    let username = crate::routes::auth::current_session_from_headers(state.as_ref(), &headers)
        .await
        .map(|session| session.username)
        .unwrap_or_else(|| "unknown".to_string());

    match state
        .ebpf_loader
        .detach_for_user(&username, payload.pin_path.as_deref())
        .await
    {
        Ok(detached) => {
            let (clean, safety_notes) = evaluate_detach_safety(
                state.as_ref(),
                &username,
                payload.pin_path.as_deref(),
                &detached,
            )
            .await;
            let severity = if clean {
                EventSeverity::Success
            } else {
                EventSeverity::Warning
            };

            state
                .event_bus
                .publish(Event {
                    username,
                    timestamp: Utc::now(),
                    source: "module-ebpf".to_string(),
                    event_type: "ebpf.detached".to_string(),
                    category: EventCategory::Platform,
                    severity,
                    color: severity.color(),
                    payload: json!({
                        "detached": detached,
                        "clean": clean,
                        "safety_notes": safety_notes,
                    }),
                })
                .await;

            (
                StatusCode::OK,
                Json(EbpfDetachResponse {
                    ok: true,
                    message: if clean {
                        "detached cleanly".to_string()
                    } else {
                        "detached with safety warnings".to_string()
                    },
                    detached,
                    clean,
                    safety_notes,
                }),
            )
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(EbpfDetachResponse {
                ok: false,
                message: error,
                detached: Vec::new(),
                clean: false,
                safety_notes: vec!["detach failed".to_string()],
            }),
        ),
    }
}
