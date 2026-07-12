async fn evaluate_detach_safety(
    state: &AppState,
    username: &str,
    requested_pin: Option<&str>,
    detached: &[String],
) -> (bool, Vec<String>) {
    let mut notes = Vec::new();

    for path in detached {
        if fs_path_exists(path).await {
            notes.push(format!("pin path still exists after detach: {path}"));
        }
    }

    let remaining = state.ebpf_loader.list_attachments_for_user(username).await;
    for path in detached {
        if remaining.iter().any(|item| item == path) {
            notes.push(format!("pin path still tracked in attachment set: {path}"));
        }
    }

    if requested_pin.is_none() && !remaining.is_empty() {
        notes.push(format!(
            "detach all requested but {} attachment(s) remain",
            remaining.len()
        ));
    }

    if notes.is_empty() {
        (true, Vec::new())
    } else {
        (false, notes)
    }
}

async fn fs_path_exists(path: &str) -> bool {
    tokio::fs::metadata(path).await.is_ok()
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

async fn stream_kernel_events(
    ebpf_loader: crate::services::ebpf_loader::EbpfLoader,
    event_bus: crate::services::event_bus::EventBus,
    username: String,
    program_name: String,
    template_id: Option<String>,
    code: String,
    pin_path: Option<String>,
    runtime_backend: EbpfRuntimeBackend,
    sample_per_sec: u32,
    stream_seconds: u32,
) {
    if code.contains("tracepoint/sched/sched_switch") {
        spawn_sched_switch_stimulus(stream_seconds);
    }

    if is_ringbuf_program(&code) {
        let preferred_map = extract_ringbuf_map_name(&code).unwrap_or_else(|| "events".to_string());
        if runtime_backend == EbpfRuntimeBackend::Aya {
            if let Some(pin_path) = pin_path.clone() {
                if stream_aya_ringbuf_events(
                    ebpf_loader.clone(),
                    event_bus.clone(),
                    username.clone(),
                    program_name.clone(),
                    template_id.clone(),
                    pin_path,
                    preferred_map.clone(),
                    sample_per_sec,
                    stream_seconds,
                )
                .await
                {
                    return;
                }
            }
        }
        if stream_ringbuf_events(
            event_bus.clone(),
            username.clone(),
            program_name.clone(),
            template_id.clone(),
            pin_path,
            preferred_map,
            sample_per_sec,
            stream_seconds,
        )
        .await
        {
            return;
        }
    }

    if !stream_kernel_trace_events(
        event_bus.clone(),
        username.clone(),
        program_name.clone(),
        template_id.clone(),
        sample_per_sec,
        stream_seconds,
    )
    .await
    {
        event_bus
            .publish(Event {
                username,
                timestamp: Utc::now(),
                source: "module-ebpf".to_string(),
                event_type: "ebpf.kernel_stream_empty".to_string(),
                category: EventCategory::Kernel,
                severity: EventSeverity::Warning,
                color: EventSeverity::Warning.color(),
                payload: json!({
                    "message": "No kernel events captured in sampling window. Program may not be attached or trigger conditions were not met.",
                    "program_name": program_name,
                    "template_id": template_id,
                    "sampling_per_sec": sample_per_sec,
                    "stream_seconds": stream_seconds,
                }),
            })
            .await;
    }
}

async fn stream_aya_ringbuf_events(
    ebpf_loader: crate::services::ebpf_loader::EbpfLoader,
    event_bus: crate::services::event_bus::EventBus,
    username: String,
    program_name: String,
    template_id: Option<String>,
    pin_path: String,
    preferred_map_name: String,
    sample_per_sec: u32,
    stream_seconds: u32,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(stream_seconds as u64);
    let sample_interval = Duration::from_millis((1000 / sample_per_sec.max(1)) as u64);
    let mut next_allowed = Instant::now();
    let mut received_any = false;

    while Instant::now() < deadline {
        let polled = match ebpf_loader
            .poll_aya_ringbuf(&pin_path, &preferred_map_name, 64)
            .await
        {
            Ok(events) => events,
            Err(error) => {
                event_bus
                    .publish(Event {
                        username: username.clone(),
                        timestamp: Utc::now(),
                        source: "module-ebpf".to_string(),
                        event_type: "ebpf.kernel_ringbuf_error".to_string(),
                        category: EventCategory::Kernel,
                        severity: EventSeverity::Warning,
                        color: EventSeverity::Warning.color(),
                        payload: json!({
                            "message": "aya ringbuf poll failed",
                            "error": error,
                            "program_name": program_name,
                            "template_id": template_id,
                        }),
                    })
                    .await;
                return false;
            }
        };

        for data in polled {
            received_any = true;
            if Instant::now() < next_allowed {
                continue;
            }
            next_allowed = Instant::now() + sample_interval;
            event_bus
                .publish(Event {
                    username: username.clone(),
                    timestamp: Utc::now(),
                    source: "module-ebpf".to_string(),
                    event_type: "ebpf.kernel_ringbuf".to_string(),
                    category: EventCategory::Kernel,
                    severity: EventSeverity::Success,
                    color: EventSeverity::Success.color(),
                    payload: json!({
                        "bytes": data.len(),
                        "preview_hex": hex_preview(&data, 64),
                        "program_name": program_name,
                        "template_id": template_id,
                        "sampling_per_sec": sample_per_sec,
                    }),
                })
                .await;
        }

        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    received_any
}

fn spawn_sched_switch_stimulus(stream_seconds: u32) {
    let rounds = std::cmp::max(1, stream_seconds);
    for _ in 0..4 {
        tokio::spawn(async move {
            let stop_at = Instant::now() + Duration::from_secs(rounds as u64);
            while Instant::now() < stop_at {
                tokio::task::yield_now().await;
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
    }
}

