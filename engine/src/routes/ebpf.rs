use std::{
    path::Path,
    sync::{Arc, OnceLock},
};

use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
use chrono::Utc;
use serde_json::{json, Value};
use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
    sync::Semaphore,
    time::{Duration, Instant},
};

use crate::{
    models::{
        ebpf::{
            EbpfAttachmentDetail, EbpfAttachmentDetailListResponse, EbpfAttachmentListResponse,
            EbpfCheckResponse, EbpfCompletionRequest, EbpfCompletionResponse, EbpfDetachRequest,
            EbpfDetachResponse, EbpfRunRequest, EbpfRunResponse, EbpfRuntimeBackend, EbpfTemplate,
        },
        event::{Event, EventCategory, EventSeverity},
    },
    services::{runner_driver::RunnerExecutionRequest, runner_manager::RunnerExecutionError},
    AppState,
};

const MAX_EBPF_SOURCE_BYTES: usize = 256 * 1024;
static EBPF_CHECK_SLOTS: OnceLock<Semaphore> = OnceLock::new();
static EBPF_COMPLETION_SLOTS: OnceLock<Semaphore> = OnceLock::new();

include!("ebpf/handlers.inc.rs");
include!("ebpf/learning.inc.rs");
include!("ebpf/check.inc.rs");
include!("ebpf/completion.inc.rs");
include!("ebpf/stream.inc.rs");
include!("ebpf/ringbuf.inc.rs");
include!("ebpf/templates.inc.rs");
