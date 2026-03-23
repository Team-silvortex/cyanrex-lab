use std::{path::Path, sync::Arc};

use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
use chrono::Utc;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    time::{Duration, Instant},
};

use crate::{
    models::{
        ebpf::{EbpfRunRequest, EbpfRunResponse, EbpfTemplate},
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
            }),
        })
        .await;

    let result = state.ebpf_loader.run(&payload.code).await;

    if result.success && enable_kernel_stream {
        let event_bus = state.event_bus.clone();
        let username_for_stream = username.clone();
        let pin_path = result.pin_path.clone();
        let code = payload.code.clone();
        tokio::spawn(async move {
            stream_kernel_events(
                event_bus,
                username_for_stream,
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

    stream_kernel_trace_events(event_bus, username, sample_per_sec, stream_seconds).await;
}

async fn stream_kernel_trace_events(
    event_bus: crate::services::event_bus::EventBus,
    username: String,
    sample_per_sec: u32,
    stream_seconds: u32,
) {
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
            return;
        }
    };

    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill().await;
        return;
    };

    let mut lines = BufReader::new(stdout).lines();
    let deadline = Instant::now() + Duration::from_secs(stream_seconds as u64);
    let sample_interval = Duration::from_millis((1000 / sample_per_sec.max(1)) as u64);
    let mut next_allowed = Instant::now();

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
}

async fn stream_ringbuf_events(
    event_bus: crate::services::event_bus::EventBus,
    username: String,
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
                            event_type: "ebpf.kernel_ringbuf".to_string(),
                            category: EventCategory::Kernel,
                            severity: EventSeverity::Success,
                            color: EventSeverity::Success.color(),
                            payload: json!({
                                "line": line,
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
    ]
}
