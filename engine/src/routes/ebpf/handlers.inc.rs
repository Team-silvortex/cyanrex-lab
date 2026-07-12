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
    let runtime_backend = payload.runtime_backend.unwrap_or(EbpfRuntimeBackend::Bpftool);

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
            }),
        })
        .await;

    let result = state
        .ebpf_loader
        .run(&username, &payload.code, Some(program_name), runtime_backend)
        .await;

    let mut attach_verified = false;
    let mut attach_expected = false;

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

    if result.success && enable_kernel_stream && attach_verified {
        let event_bus = state.event_bus.clone();
        let ebpf_loader = state.ebpf_loader.clone();
        let username_for_stream = username.clone();
        let program_name_for_stream = program_name.to_string();
        let template_id_for_stream = template_id.clone();
        let pin_path = result.pin_path.clone();
        let code = payload.code.clone();
        let runtime_backend_for_stream = runtime_backend;
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
            )
            .await;
        });
    } else if result.success && enable_kernel_stream && attach_expected && !attach_verified {
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

