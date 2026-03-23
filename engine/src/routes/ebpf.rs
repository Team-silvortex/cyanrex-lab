use std::{path::Path, sync::Arc};

use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
use chrono::Utc;
use serde_json::{json, Value};
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
    time::{Duration, Instant},
};

use crate::{
    models::{
        ebpf::{
            EbpfAttachmentDetail, EbpfAttachmentDetailListResponse, EbpfAttachmentListResponse,
            EbpfDetachRequest, EbpfDetachResponse, EbpfRunRequest, EbpfRunResponse, EbpfTemplate,
        },
        event::{Event, EventCategory, EventSeverity},
    },
    AppState,
};

const MAX_EBPF_SOURCE_BYTES: usize = 256 * 1024;

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
            }),
        })
        .await;

    let result = state
        .ebpf_loader
        .run(&username, &payload.code, Some(program_name))
        .await;

    if result.success {
        let attach_check = verify_attach_state(
            result.pin_path.as_deref(),
            &payload.code,
            &result.load_stderr,
        )
        .await;
        let expect_attach = expects_autoattach(&payload.code);
        let (event_type, severity, message) = if attach_check.attached {
            (
                "ebpf.attach_verified",
                EventSeverity::Success,
                "eBPF attachment verified".to_string(),
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
                    "expected_autoattach": expect_attach,
                    "attached": attach_check.attached,
                    "reason": attach_check.reason,
                    "program_ids": attach_check.program_ids,
                    "linked_program_ids": attach_check.linked_program_ids,
                }),
            })
            .await;
    }

    if result.success && enable_kernel_stream {
        let event_bus = state.event_bus.clone();
        let username_for_stream = username.clone();
        let program_name_for_stream = program_name.to_string();
        let template_id_for_stream = template_id.clone();
        let pin_path = result.pin_path.clone();
        let code = payload.code.clone();
        tokio::spawn(async move {
            stream_kernel_events(
                event_bus,
                username_for_stream,
                program_name_for_stream,
                template_id_for_stream,
                code,
                pin_path,
                sample_per_sec,
                stream_seconds,
            )
            .await;
        });
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
    event_bus: crate::services::event_bus::EventBus,
    username: String,
    program_name: String,
    template_id: Option<String>,
    code: String,
    pin_path: Option<String>,
    sample_per_sec: u32,
    stream_seconds: u32,
) {
    if is_ringbuf_program(&code) {
        let preferred_map = extract_ringbuf_map_name(&code).unwrap_or_else(|| "events".to_string());
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
    for map in maps {
        let map_type = map.get("type").and_then(Value::as_str).unwrap_or_default();
        if map_type != "ringbuf" {
            continue;
        }
        let id = map.get("id").and_then(Value::as_i64)?;
        let name = map.get("name").and_then(Value::as_str).unwrap_or_default();
        if name == preferred_map_name {
            return Some(RingbufTarget::Id(id));
        }
        fallback_id = Some(id);
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

async fn verify_attach_state(pin_path: Option<&str>, code: &str, load_stderr: &str) -> AttachCheck {
    let Some(pin_path) = pin_path else {
        return AttachCheck {
            attached: false,
            reason: "missing pin_path from loader result".to_string(),
            program_ids: Vec::new(),
            linked_program_ids: Vec::new(),
        };
    };

    let lower_stderr = load_stderr.to_ascii_lowercase();
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

    let reason = if attached {
        "program id matched active bpf link".to_string()
    } else if expects_autoattach(code) {
        if autoattach_unsupported {
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

fn default_templates() -> Vec<EbpfTemplate> {
    vec![
        EbpfTemplate {
            id: "xdp-pass".to_string(),
            name: "XDP Pass".to_string(),
            description: "最小 XDP 程序，适合验证编译/加载链路".to_string(),
            capability: "xdp".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

SEC("xdp")
int xdp_pass(struct xdp_md *ctx) {
  return XDP_PASS;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "tracepoint-sys-enter".to_string(),
            name: "Tracepoint Sys Enter".to_string(),
            description: "tracepoint 事件，输出内核日志（可在 events 查看采样流）".to_string(),
            capability: "tracepoint".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

SEC("tracepoint/syscalls/sys_enter_execve")
int on_execve(void *ctx) {
  bpf_printk("execve entered");
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "ringbuf-skeleton".to_string(),
            name: "Ringbuf Skeleton".to_string(),
            description: "ringbuf 结构模板（用户态 reader 可按此 map 进行消费）".to_string(),
            capability: "ringbuf".to_string(),
            code: r#"#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct event_t {
  __u64 ts;
  __u32 pid;
};

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} events SEC(".maps");

SEC("tracepoint/syscalls/sys_enter_execve")
int on_execve(void *ctx) {
  struct event_t *evt = bpf_ringbuf_reserve(&events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }
  evt->ts = bpf_ktime_get_ns();
  evt->pid = bpf_get_current_pid_tgid() >> 32;
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
        EbpfTemplate {
            id: "ringbuf-hi-freq-sampler".to_string(),
            name: "Ringbuf High-Freq Sampler".to_string(),
            description:
                "高频 tracepoint 切面 + 内核侧采样节流（默认每 64 次上报 1 次），用于展示事件流能力且不干扰系统".to_string(),
            capability: "ringbuf".to_string(),
            code: r#"#include <vmlinux.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct event_t {
  __u64 ts;
  __u64 count;
  __u32 pid;
  __u32 cpu;
};

struct {
  __uint(type, BPF_MAP_TYPE_RINGBUF);
  __uint(max_entries, 1 << 24);
} events SEC(".maps");

struct {
  __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
  __uint(max_entries, 1);
  __type(key, __u32);
  __type(value, __u64);
} per_cpu_counter SEC(".maps");

SEC("tracepoint/sched/sched_switch")
int on_sched_switch(struct trace_event_raw_sched_switch *ctx) {
  __u32 key = 0;
  __u64 *counter = bpf_map_lookup_elem(&per_cpu_counter, &key);
  if (!counter) {
    return 0;
  }

  *counter += 1;
  if ((*counter & 63) != 0) {
    return 0;
  }

  struct event_t *evt = bpf_ringbuf_reserve(&events, sizeof(*evt), 0);
  if (!evt) {
    return 0;
  }

  evt->ts = bpf_ktime_get_ns();
  evt->count = *counter;
  evt->pid = ctx->next_pid;
  evt->cpu = bpf_get_smp_processor_id();
  bpf_ringbuf_submit(evt, 0);
  return 0;
}

char _license[] SEC("license") = "GPL";"#
                .to_string(),
        },
    ]
}
