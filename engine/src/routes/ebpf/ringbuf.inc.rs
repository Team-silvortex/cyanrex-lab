async fn stream_kernel_trace_events(
    event_bus: crate::services::event_bus::EventBus,
    username: String,
    program_name: String,
    template_id: Option<String>,
    sample_per_sec: u32,
    stream_seconds: u32,
) -> bool {
    let mut child = match Command::new("bpftool")
        .arg("prog")
        .arg("tracelog")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(process) => process,
        Err(error) => {
            event_bus
                .publish(Event {
                    username,
                    timestamp: Utc::now(),
                    source: "module-ebpf".to_string(),
                    event_type: "ebpf.kernel_stream_error".to_string(),
                    category: EventCategory::Kernel,
                    severity: EventSeverity::Error,
                    color: EventSeverity::Error.color(),
                    payload: json!({
                        "message": format!("failed to start bpftool tracelog: {error}"),
                    }),
                })
                .await;
            return false;
        }
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill().await;
        return false;
    };

    let mut lines = BufReader::new(stdout).lines();
    let deadline = Instant::now() + Duration::from_secs(stream_seconds as u64);
    let sample_interval = Duration::from_millis((1000 / sample_per_sec.max(1)) as u64);
    let mut next_allowed = Instant::now();
    let mut received_any = false;

    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => break,
            maybe_line = lines.next_line() => {
                match maybe_line {
                    Ok(Some(line)) => {
                        if Instant::now() < next_allowed {
                            continue;
                        }
                        next_allowed = Instant::now() + sample_interval;
                        received_any = true;
                        event_bus.publish(Event {
                            username: username.clone(),
                            timestamp: Utc::now(),
                            source: "module-ebpf".to_string(),
                            event_type: "ebpf.kernel_trace".to_string(),
                            category: EventCategory::Kernel,
                            severity: EventSeverity::Success,
                            color: EventSeverity::Success.color(),
                            payload: json!({
                                "line": line,
                                "program_name": program_name,
                                "template_id": template_id,
                                "sampling_per_sec": sample_per_sec,
                            }),
                        }).await;
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }
    }

    let _ = child.kill().await;
    received_any
}

#[allow(clippy::too_many_arguments)]
async fn stream_ringbuf_events(
    event_bus: crate::services::event_bus::EventBus,
    username: String,
    program_name: String,
    template_id: Option<String>,
    pin_path: Option<String>,
    preferred_map_name: String,
    sample_per_sec: u32,
    stream_seconds: u32,
) -> bool {
    let target = match resolve_ringbuf_target(pin_path, &preferred_map_name).await {
        Some(value) => value,
        None => return false,
    };

    let mut command = Command::new("bpftool");
    command.arg("map").arg("event_pipe");
    match target {
        RingbufTarget::Id(id) => {
            command.arg("id").arg(id.to_string());
        }
        RingbufTarget::Pinned(path) => {
            command.arg("pinned").arg(path);
        }
    }
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = match command.spawn() {
        Ok(process) => process,
        Err(_) => return false,
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill().await;
        return false;
    };

    let mut stdout = stdout;
    let deadline = Instant::now() + Duration::from_secs(stream_seconds as u64);
    let sample_interval = Duration::from_millis((1000 / sample_per_sec.max(1)) as u64);
    let mut next_allowed = Instant::now();
    let mut received_any = false;
    let mut chunk = vec![0_u8; 4096];

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }

        let wait = std::cmp::min(
            Duration::from_millis(200),
            deadline.saturating_duration_since(now),
        );
        match tokio::time::timeout(wait, stdout.read(&mut chunk)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(size)) => {
                received_any = true;
                if Instant::now() < next_allowed {
                    continue;
                }
                next_allowed = Instant::now() + sample_interval;
                let data = &chunk[..size];
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
                            "bytes": size,
                            "preview_hex": hex_preview(data, 64),
                            "program_name": program_name,
                            "template_id": template_id,
                            "sampling_per_sec": sample_per_sec,
                        }),
                    })
                    .await;
            }
            Ok(Err(_)) => break,
            Err(_) => {}
        }
    }

    let _ = child.kill().await;
    if let Ok(output) = child.wait_with_output().await {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stderr.is_empty() && !received_any {
            event_bus
                .publish(Event {
                    username,
                    timestamp: Utc::now(),
                    source: "module-ebpf".to_string(),
                    event_type: "ebpf.kernel_ringbuf_error".to_string(),
                    category: EventCategory::Kernel,
                    severity: EventSeverity::Warning,
                    color: EventSeverity::Warning.color(),
                    payload: json!({
                        "message": "ringbuf event pipe returned no data",
                        "stderr": stderr,
                        "program_name": program_name,
                        "template_id": template_id,
                    }),
                })
                .await;
        }
    }
    received_any
}

fn hex_preview(bytes: &[u8], max_len: usize) -> String {
    let mut output = String::new();
    for (idx, byte) in bytes.iter().take(max_len).enumerate() {
        if idx > 0 {
            output.push(' ');
        }
        output.push_str(&format!("{byte:02x}"));
    }
    if bytes.len() > max_len {
        output.push_str(" ...");
    }
    output
}

enum RingbufTarget {
    Id(i64),
    Pinned(String),
}

async fn resolve_ringbuf_target(
    pin_path: Option<String>,
    preferred_map_name: &str,
) -> Option<RingbufTarget> {
    if let Some(base) = pin_path {
        let direct = format!("{base}/{preferred_map_name}");
        if Path::new(&direct).exists() {
            return Some(RingbufTarget::Pinned(direct));
        }
        let nested = format!("{base}/maps/{preferred_map_name}");
        if Path::new(&nested).exists() {
            return Some(RingbufTarget::Pinned(nested));
        }
    }

    let output = Command::new("bpftool")
        .arg("-j")
        .arg("map")
        .arg("show")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value: Value = serde_json::from_slice(&output.stdout).ok()?;
    let maps = value.as_array()?;

    let mut fallback_id = None;
    let mut preferred_id = None;
    for map in maps {
        let map_type = map.get("type").and_then(Value::as_str).unwrap_or_default();
        if map_type != "ringbuf" {
            continue;
        }
        let id = map.get("id").and_then(Value::as_i64)?;
        let name = map.get("name").and_then(Value::as_str).unwrap_or_default();
        if name == preferred_map_name {
            preferred_id = Some(preferred_id.map_or(id, |curr: i64| curr.max(id)));
            continue;
        }
        fallback_id = Some(fallback_id.map_or(id, |curr: i64| curr.max(id)));
    }

    if let Some(id) = preferred_id {
        return Some(RingbufTarget::Id(id));
    }

    fallback_id.map(RingbufTarget::Id)
}

fn is_ringbuf_program(code: &str) -> bool {
    code.contains("BPF_MAP_TYPE_RINGBUF") || code.contains("bpf_ringbuf_")
}

fn extract_ringbuf_map_name(code: &str) -> Option<String> {
    let marker = "SEC(\".maps\")";
    let idx = code.find(marker)?;
    let left = &code[..idx];
    let mut token = String::new();

    for ch in left.chars().rev() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            token.push(ch);
        } else if !token.is_empty() {
            break;
        }
    }

    if token.is_empty() {
        return None;
    }

    Some(token.chars().rev().collect())
}

struct AttachCheck {
    attached: bool,
    reason: String,
    program_ids: Vec<i64>,
    linked_program_ids: Vec<i64>,
}

fn expects_autoattach(code: &str) -> bool {
    code.contains("SEC(\"tracepoint/")
        || code.contains("SEC(\"raw_tracepoint/")
        || code.contains("SEC(\"kprobe/")
        || code.contains("SEC(\"kretprobe/")
}

async fn verify_attach_state(
    pin_path: Option<&str>,
    code: &str,
    load_output: &str,
    run_message: &str,
    runtime_backend: EbpfRuntimeBackend,
) -> AttachCheck {
    if runtime_backend == EbpfRuntimeBackend::Aya {
        let lower_output = load_output.to_ascii_lowercase();
        let attached = lower_output.contains("aya attach success")
            || run_message
                .to_ascii_lowercase()
                .contains("attached successfully (aya backend)");
        let reason = if attached {
            "aya attach reported success".to_string()
        } else {
            "aya attach success marker not found in loader output/message".to_string()
        };
        return AttachCheck {
            attached,
            reason,
            program_ids: Vec::new(),
            linked_program_ids: Vec::new(),
        };
    }

    let Some(pin_path) = pin_path else {
        return AttachCheck {
            attached: false,
            reason: "missing pin_path from loader result".to_string(),
            program_ids: Vec::new(),
            linked_program_ids: Vec::new(),
        };
    };

    let lower_stderr = load_output.to_ascii_lowercase();
    let autoattach_unsupported = lower_stderr.contains("autoattach")
        && (lower_stderr.contains("unknown")
            || lower_stderr.contains("invalid")
            || lower_stderr.contains("unrecognized"));

    let program_ids = collect_prog_ids_from_pin(pin_path).await;
    if program_ids.is_empty() {
        return AttachCheck {
            attached: false,
            reason: "no pinned program ids found under pin_path".to_string(),
            program_ids,
            linked_program_ids: Vec::new(),
        };
    }

    let linked_program_ids = collect_linked_prog_ids().await;
    let attached = program_ids
        .iter()
        .any(|id| linked_program_ids.iter().any(|linked| linked == id));

    let manual_tracepoint_attach_unsupported = lower_stderr
        .contains("manual attach skipped: current bpftool does not support tracepoint attach");

    let reason = if attached {
        "program id matched active bpf link".to_string()
    } else if expects_autoattach(code) {
        if autoattach_unsupported && manual_tracepoint_attach_unsupported {
            "both autoattach and manual tracepoint attach are unsupported by current bpftool"
                .to_string()
        } else if autoattach_unsupported {
            "autoattach unsupported and no manual attach link matched pinned program ids"
                .to_string()
        } else {
            "no active bpf link matched pinned program ids".to_string()
        }
    } else {
        "no link match; this program type may need manual attach target".to_string()
    };

    AttachCheck {
        attached,
        reason,
        program_ids,
        linked_program_ids,
    }
}

async fn collect_prog_ids_from_pin(pin_path: &str) -> Vec<i64> {
    let meta = match fs::metadata(pin_path).await {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    let mut ids = Vec::new();
    if meta.is_dir() {
        let mut reader = match fs::read_dir(pin_path).await {
            Ok(value) => value,
            Err(_) => return Vec::new(),
        };

        while let Ok(Some(entry)) = reader.next_entry().await {
            let path = entry.path();
            if let Some(path_str) = path.to_str() {
                ids.extend(prog_ids_for_pinned_path(path_str).await);
            }
        }
    } else {
        ids.extend(prog_ids_for_pinned_path(pin_path).await);
    }

    ids.sort_unstable();
    ids.dedup();
    ids
}

async fn prog_ids_for_pinned_path(path: &str) -> Vec<i64> {
    let output = match Command::new("bpftool")
        .arg("-j")
        .arg("prog")
        .arg("show")
        .arg("pinned")
        .arg(path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
    {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    if !output.status.success() {
        return Vec::new();
    }

    extract_numeric_field_ids(&output.stdout, "id")
}

async fn collect_linked_prog_ids() -> Vec<i64> {
    let output = match Command::new("bpftool")
        .arg("-j")
        .arg("link")
        .arg("show")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
    {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    if !output.status.success() {
        return Vec::new();
    }

    extract_numeric_field_ids(&output.stdout, "prog_id")
}

fn extract_numeric_field_ids(bytes: &[u8], field: &str) -> Vec<i64> {
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    if let Some(arr) = value.as_array() {
        for item in arr {
            if let Some(id) = item.get(field).and_then(Value::as_i64) {
                out.push(id);
            }
        }
        return out;
    }

    if let Some(id) = value.get(field).and_then(Value::as_i64) {
        out.push(id);
    }
    out
}
